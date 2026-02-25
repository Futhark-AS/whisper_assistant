use std::process::Command;

use chrono::Utc;
use regex::Regex;

use crate::bootstrap::AppPaths;
use crate::capture::devices::list_input_devices;
use crate::config::AppConfig;
use crate::doctor::report::{CheckResult, CheckStatus, DoctorReport, DoctorState};

pub fn run_doctor(paths: &AppPaths, config: &AppConfig) -> DoctorReport {
    let mut checks = vec![
        check_binary_version(
            "ffmpeg",
            "6.0",
            true,
            Some("Install ffmpeg via your package manager."),
        ),
        check_binary_version(
            "ffprobe",
            "6.0",
            true,
            Some("Install ffmpeg package, which includes ffprobe."),
        ),
        check_binary_version(
            "whisper-cli",
            "1.7.2",
            true,
            Some("Install whisper.cpp and ensure whisper-cli is in PATH."),
        ),
        check_binary_version(
            "insanely-fast-whisper",
            "0.0.15",
            false,
            Some("Install with pipx install insanely-fast-whisper if you want fallback backend."),
        ),
    ];

    let python_required = config.transcription.diarize;
    checks.push(check_binary_version(
        "python3",
        "3.10",
        python_required,
        Some("Install python3 >= 3.10 for diarization backend support."),
    ));

    checks.push(check_microphone_permission(
        config.permissions.microphone_required,
    ));
    checks.push(check_recording_backend_capability());
    checks.extend(check_macos_metal(paths));

    let state = derive_state(&checks);

    DoctorReport {
        generated_at_rfc3339: Utc::now().to_rfc3339(),
        state,
        checks,
    }
}

fn derive_state(checks: &[CheckResult]) -> DoctorState {
    let required_failed = checks
        .iter()
        .any(|check| check.required && check.status == CheckStatus::Fail);
    let any_degraded = checks
        .iter()
        .any(|check| matches!(check.status, CheckStatus::Warn | CheckStatus::Fail));

    if required_failed {
        DoctorState::Unavailable
    } else if any_degraded {
        DoctorState::Degraded
    } else {
        DoctorState::Ready
    }
}

fn check_recording_backend_capability() -> CheckResult {
    match list_input_devices() {
        Ok(devices) if devices.is_empty() => CheckResult {
            name: "recording_backend".to_owned(),
            status: CheckStatus::Warn,
            detail: "no recording devices discovered".to_owned(),
            required: true,
            remediation: Some(
                "Connect a microphone and verify audio subsystem configuration.".to_owned(),
            ),
        },
        Ok(devices) => CheckResult {
            name: "recording_backend".to_owned(),
            status: CheckStatus::Pass,
            detail: format!("{} device(s) discovered", devices.len()),
            required: true,
            remediation: None,
        },
        Err(error) => CheckResult {
            name: "recording_backend".to_owned(),
            status: CheckStatus::Fail,
            detail: format!("recording backend unavailable: {error}"),
            required: true,
            remediation: Some(
                "Install/enable `arecord` or `ffmpeg` recording support for Linux capture."
                    .to_owned(),
            ),
        },
    }
}

fn check_binary_version(
    binary: &str,
    min_version: &str,
    required: bool,
    remediation: Option<&str>,
) -> CheckResult {
    let missing = || CheckResult {
        name: binary.to_owned(),
        status: CheckStatus::Fail,
        detail: "binary not found in PATH".to_owned(),
        required,
        remediation: remediation.map(ToOwned::to_owned),
    };

    let path = match which::which(binary) {
        Ok(path) => path,
        Err(_) => return missing(),
    };

    let output = version_output(binary);
    let parsed = output.as_deref().and_then(parse_version_triplet);

    match parsed {
        Some(found) => {
            if version_at_least(&found, &parse_target_version(min_version)) {
                CheckResult {
                    name: binary.to_owned(),
                    status: CheckStatus::Pass,
                    detail: format!(
                        "{} (>= {}) at {}",
                        version_triplet_string(&found),
                        min_version,
                        path.display()
                    ),
                    required,
                    remediation: None,
                }
            } else {
                CheckResult {
                    name: binary.to_owned(),
                    status: CheckStatus::Fail,
                    detail: format!("{} (< {})", version_triplet_string(&found), min_version),
                    required,
                    remediation: remediation.map(ToOwned::to_owned),
                }
            }
        }
        None => CheckResult {
            name: binary.to_owned(),
            status: CheckStatus::Warn,
            detail: format!("installed at {}, version parse failed", path.display()),
            required,
            remediation: remediation.map(ToOwned::to_owned),
        },
    }
}

fn version_output(binary: &str) -> Option<String> {
    let variants = [["--version"], ["-V"], ["version"]];

    for args in variants {
        let output = Command::new(binary).args(args).output().ok()?;
        let text = if output.stdout.is_empty() {
            String::from_utf8_lossy(&output.stderr).to_string()
        } else {
            String::from_utf8_lossy(&output.stdout).to_string()
        };
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    None
}

fn parse_version_triplet(text: &str) -> Option<[u32; 3]> {
    let regex = Regex::new(r"(?P<a>\d+)\.(?P<b>\d+)(?:\.(?P<c>\d+))?").ok()?;
    let captures = regex.captures(text)?;

    let major = captures.name("a")?.as_str().parse::<u32>().ok()?;
    let minor = captures.name("b")?.as_str().parse::<u32>().ok()?;
    let patch = captures
        .name("c")
        .map(|m| m.as_str().parse::<u32>().ok())
        .unwrap_or(Some(0))?;

    Some([major, minor, patch])
}

fn parse_target_version(text: &str) -> [u32; 3] {
    let mut parts = text
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok())
        .collect::<Vec<_>>();
    while parts.len() < 3 {
        parts.push(0);
    }

    [parts[0], parts[1], parts[2]]
}

fn version_at_least(found: &[u32; 3], required: &[u32; 3]) -> bool {
    found >= required
}

fn version_triplet_string(value: &[u32; 3]) -> String {
    format!("{}.{}.{}", value[0], value[1], value[2])
}

fn check_microphone_permission(required: bool) -> CheckResult {
    #[cfg(target_os = "macos")]
    {
        if which::which("swift").is_err() {
            return CheckResult {
                name: "microphone_permission".to_owned(),
                status: if required {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Skip
                },
                detail: "swift not available to query AVFoundation authorization status".to_owned(),
                required,
                remediation: Some("Install Xcode command line tools and rerun doctor.".to_owned()),
            };
        }

        let script = r#"
import AVFoundation
let status = AVCaptureDevice.authorizationStatus(for: .audio)
print(status.rawValue)
"#;

        let output = Command::new("swift").arg("-e").arg(script).output();
        match output {
            Ok(output) => {
                let raw = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                match raw.as_str() {
                    "3" => CheckResult {
                        name: "microphone_permission".to_owned(),
                        status: CheckStatus::Pass,
                        detail: "authorized".to_owned(),
                        required,
                        remediation: None,
                    },
                    "0" => CheckResult {
                        name: "microphone_permission".to_owned(),
                        status: CheckStatus::Warn,
                        detail: "not determined".to_owned(),
                        required,
                        remediation: Some(
                            "Start Quedo and grant microphone access when prompted in System Settings."
                                .to_owned(),
                        ),
                    },
                    "1" | "2" => CheckResult {
                        name: "microphone_permission".to_owned(),
                        status: CheckStatus::Fail,
                        detail: "denied/restricted".to_owned(),
                        required,
                        remediation: Some(
                            "Open System Settings -> Privacy & Security -> Microphone and allow Quedo."
                                .to_owned(),
                        ),
                    },
                    _ => CheckResult {
                        name: "microphone_permission".to_owned(),
                        status: CheckStatus::Warn,
                        detail: format!("unexpected permission status `{raw}`"),
                        required,
                        remediation: Some("Retry permission probe with `quedo-daemon doctor`.".to_owned()),
                    },
                }
            }
            Err(error) => CheckResult {
                name: "microphone_permission".to_owned(),
                status: if required {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Skip
                },
                detail: format!("permission probe command failed: {error}"),
                required,
                remediation: Some("Verify Swift toolchain availability.".to_owned()),
            },
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let probe = which::which("arecord");
        match probe {
            Ok(_) => {
                let output = Command::new("arecord").arg("-l").output();
                match output {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if stdout.to_ascii_lowercase().contains("card") {
                            CheckResult {
                                name: "microphone_probe".to_owned(),
                                status: CheckStatus::Pass,
                                detail: "capture devices detected via arecord -l".to_owned(),
                                required,
                                remediation: None,
                            }
                        } else {
                            CheckResult {
                                name: "microphone_probe".to_owned(),
                                status: if required {
                                    CheckStatus::Warn
                                } else {
                                    CheckStatus::Skip
                                },
                                detail: "no input devices listed".to_owned(),
                                required,
                                remediation: Some(
                                    "Connect a microphone or verify ALSA/PulseAudio device routing."
                                        .to_owned(),
                                ),
                            }
                        }
                    }
                    Err(error) => CheckResult {
                        name: "microphone_probe".to_owned(),
                        status: if required {
                            CheckStatus::Warn
                        } else {
                            CheckStatus::Skip
                        },
                        detail: format!("failed to execute arecord -l: {error}"),
                        required,
                        remediation: Some("Install ALSA utils for input probing.".to_owned()),
                    },
                }
            }
            Err(_) => CheckResult {
                name: "microphone_probe".to_owned(),
                status: if required {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Skip
                },
                detail: "arecord not installed; cannot probe input device availability".to_owned(),
                required,
                remediation: Some("Install `alsa-utils` and rerun doctor.".to_owned()),
            },
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn check_macos_metal(_paths: &AppPaths) -> Vec<CheckResult> {
    vec![CheckResult {
        name: "metal_backend".to_owned(),
        status: CheckStatus::Skip,
        detail: "not macOS".to_owned(),
        required: false,
        remediation: None,
    }]
}

#[cfg(target_os = "macos")]
fn check_macos_metal(paths: &AppPaths) -> Vec<CheckResult> {
    let mut results = Vec::new();

    let whisper_path = match which::which("whisper-cli") {
        Ok(path) => path,
        Err(_) => {
            results.push(CheckResult {
                name: "metal_link".to_owned(),
                status: CheckStatus::Fail,
                detail: "whisper-cli missing from PATH".to_owned(),
                required: true,
                remediation: Some("Install whisper.cpp and expose whisper-cli.".to_owned()),
            });
            return results;
        }
    };

    let link_check = Command::new("otool").arg("-L").arg(&whisper_path).output();
    match link_check {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            if text.contains("metal.framework") {
                results.push(CheckResult {
                    name: "metal_link".to_owned(),
                    status: CheckStatus::Pass,
                    detail: format!("Metal.framework linked in {}", whisper_path.display()),
                    required: true,
                    remediation: None,
                });
            } else {
                results.push(CheckResult {
                    name: "metal_link".to_owned(),
                    status: CheckStatus::Fail,
                    detail: "whisper-cli does not link Metal.framework".to_owned(),
                    required: true,
                    remediation: Some("Rebuild whisper.cpp with Metal support enabled.".to_owned()),
                });
            }
        }
        Err(error) => results.push(CheckResult {
            name: "metal_link".to_owned(),
            status: CheckStatus::Warn,
            detail: format!("failed to execute otool: {error}"),
            required: true,
            remediation: Some("Install Xcode command line tools.".to_owned()),
        }),
    }

    let model_path = std::env::var("WHISPER_MODEL_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            directories::BaseDirs::new()
                .map(|dirs| {
                    dirs.home_dir()
                        .join(".cache")
                        .join("whisper.cpp")
                        .join("ggml-base.en.bin")
                })
                .unwrap_or_else(|| paths.cache_dir.join("models").join("ggml-base.en.bin"))
        });

    if !model_path.is_file() {
        results.push(CheckResult {
            name: "metal_model".to_owned(),
            status: CheckStatus::Fail,
            detail: format!("model file not found: {}", model_path.display()),
            required: true,
            remediation: Some(
                "Set WHISPER_MODEL_PATH to a valid whisper.cpp model file.".to_owned(),
            ),
        });
        return results;
    }

    if which::which("ffmpeg").is_err() {
        results.push(CheckResult {
            name: "metal_smoke".to_owned(),
            status: CheckStatus::Fail,
            detail: "ffmpeg missing; cannot generate smoke-test audio".to_owned(),
            required: true,
            remediation: Some("Install ffmpeg and rerun doctor.".to_owned()),
        });
        return results;
    }

    let temp_dir = match tempfile::TempDir::new() {
        Ok(dir) => dir,
        Err(error) => {
            results.push(CheckResult {
                name: "metal_smoke".to_owned(),
                status: CheckStatus::Warn,
                detail: format!("unable to create temp directory: {error}"),
                required: true,
                remediation: Some("Verify temporary directory permissions.".to_owned()),
            });
            return results;
        }
    };

    let wav_path = temp_dir.path().join("metal-smoke.wav");
    let ffmpeg_result = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=16000:cl=mono",
            "-t",
            "1",
        ])
        .arg(&wav_path)
        .output();

    if ffmpeg_result.is_err() {
        results.push(CheckResult {
            name: "metal_smoke".to_owned(),
            status: CheckStatus::Fail,
            detail: "ffmpeg command failed while preparing smoke test".to_owned(),
            required: true,
            remediation: Some("Verify ffmpeg installation.".to_owned()),
        });
        return results;
    }

    let output_prefix = temp_dir.path().join("out");
    let smoke = Command::new("whisper-cli")
        .arg("-m")
        .arg(&model_path)
        .arg("-f")
        .arg(&wav_path)
        .arg("-l")
        .arg("en")
        .arg("-otxt")
        .arg("-of")
        .arg(&output_prefix)
        .output();

    match smoke {
        Ok(output) => {
            if !output.status.success() {
                results.push(CheckResult {
                    name: "metal_smoke".to_owned(),
                    status: CheckStatus::Fail,
                    detail: format!(
                        "whisper-cli smoke test failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                    required: true,
                    remediation: Some(
                        "Run whisper-cli manually to inspect backend logs.".to_owned(),
                    ),
                });
                return results;
            }

            let mut logs = String::new();
            logs.push_str(&String::from_utf8_lossy(&output.stdout).to_ascii_lowercase());
            logs.push_str(&String::from_utf8_lossy(&output.stderr).to_ascii_lowercase());
            if logs.contains("metal") || logs.contains("mps") || logs.contains("gpu") {
                results.push(CheckResult {
                    name: "metal_smoke".to_owned(),
                    status: CheckStatus::Pass,
                    detail: "whisper-cli smoke test completed with Metal markers".to_owned(),
                    required: true,
                    remediation: None,
                });
            } else {
                results.push(CheckResult {
                    name: "metal_smoke".to_owned(),
                    status: CheckStatus::Warn,
                    detail: "smoke test passed, but no explicit Metal markers were found"
                        .to_owned(),
                    required: true,
                    remediation: Some(
                        "Run with verbose whisper-cli logging to confirm GPU backend usage."
                            .to_owned(),
                    ),
                });
            }
        }
        Err(error) => results.push(CheckResult {
            name: "metal_smoke".to_owned(),
            status: CheckStatus::Fail,
            detail: format!("failed to execute whisper-cli smoke test: {error}"),
            required: true,
            remediation: Some("Verify whisper-cli installation and model path.".to_owned()),
        }),
    }

    results
}

#[cfg(test)]
mod tests {
    use super::{
        check_binary_version, check_microphone_permission, check_recording_backend_capability,
        derive_state, parse_target_version, parse_version_triplet, run_doctor, version_at_least,
    };
    use crate::bootstrap::paths::AppPaths;
    use crate::config::schema::AppConfig;
    use crate::doctor::report::{CheckResult, CheckStatus, DoctorState};
    use std::fs;
    use std::path::Path;

    struct EnvVarGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.old.as_ref() {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn write_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod");
        }
    }

    fn temp_paths(root: &Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            logs_dir: root.join("cache/logs"),
            state_dir: root.join("cache/fw-state"),
            config_file: root.join("config/config.toml"),
            history_db: root.join("data/history.sqlite3"),
            autostart_file: root.join("autostart/quedo-daemon.desktop"),
        }
    }

    #[test]
    fn version_parser_handles_two_and_three_parts() {
        assert_eq!(parse_version_triplet("ffmpeg 6.1"), Some([6, 1, 0]));
        assert_eq!(parse_version_triplet("whisper-cli 1.7.2"), Some([1, 7, 2]));
        assert_eq!(parse_version_triplet("noise"), None);
        assert!(version_at_least(&[6, 1, 0], &parse_target_version("6.0")));
        assert!(!version_at_least(&[1, 0, 0], &parse_target_version("1.7.2")));
    }

    #[test]
    fn state_derivation_matches_contract() {
        let checks = vec![CheckResult {
            name: "a".to_owned(),
            status: CheckStatus::Fail,
            detail: "bad".to_owned(),
            required: true,
            remediation: None,
        }];
        assert_eq!(derive_state(&checks), DoctorState::Unavailable);

        let checks = vec![CheckResult {
            name: "a".to_owned(),
            status: CheckStatus::Warn,
            detail: "warn".to_owned(),
            required: false,
            remediation: None,
        }];
        assert_eq!(derive_state(&checks), DoctorState::Degraded);

        let checks = vec![CheckResult {
            name: "a".to_owned(),
            status: CheckStatus::Pass,
            detail: "ok".to_owned(),
            required: true,
            remediation: None,
        }];
        assert_eq!(derive_state(&checks), DoctorState::Ready);
    }

    #[test]
    fn binary_check_missing_binary_fails() {
        let result = check_binary_version("definitely-not-a-binary", "1.0", true, Some("install"));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.detail.contains("binary not found"));
    }

    #[test]
    fn binary_check_old_version_fails() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let script = temp.path().join("mock-old");
        write_script(
            &script,
            r#"#!/bin/sh
echo "mock version 1.0.0"
"#,
        );
        let _path = EnvVarGuard::set("PATH", temp.path().to_str().expect("utf8"));
        let result = check_binary_version("mock-old", "2.0", true, Some("upgrade"));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.detail.contains("(< 2.0)"));
    }

    #[test]
    fn binary_check_acceptable_version_passes() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let script = temp.path().join("mock-ok");
        write_script(
            &script,
            r#"#!/bin/sh
echo "mock version 2.1.3"
"#,
        );
        let _path = EnvVarGuard::set("PATH", temp.path().to_str().expect("utf8"));
        let result = check_binary_version("mock-ok", "2.0", true, Some("upgrade"));
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.detail.contains("2.1.3"));
    }

    #[test]
    fn binary_check_unparseable_version_warns() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let script = temp.path().join("mock-warn");
        write_script(
            &script,
            r#"#!/bin/sh
echo "this is not a version"
"#,
        );
        let _path = EnvVarGuard::set("PATH", temp.path().to_str().expect("utf8"));
        let result = check_binary_version("mock-warn", "2.0", true, Some("upgrade"));
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.detail.contains("version parse failed"));
    }

    #[test]
    fn python_required_flag_toggles_with_diarize() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let paths = temp_paths(temp.path());
        paths.ensure_dirs().expect("dirs");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");

        for name in ["ffmpeg", "ffprobe", "whisper-cli", "python3"] {
            write_script(
                &bin.join(name),
                r#"#!/bin/sh
echo "mock version 9.9.9"
"#,
            );
        }
        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
echo "card 0: Device [Mock Device], device 0: Mock [Mock]"
"#,
        );

        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        let mut config = AppConfig::default();
        config.transcription.diarize = false;
        let report = run_doctor(&paths, &config);
        let python = report
            .checks
            .iter()
            .find(|check| check.name == "python3")
            .expect("python check");
        assert!(!python.required);

        config.transcription.diarize = true;
        let report = run_doctor(&paths, &config);
        let python = report
            .checks
            .iter()
            .find(|check| check.name == "python3")
            .expect("python check");
        assert!(python.required);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_microphone_probe_outcomes() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");

        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
echo "card 0: Device [Mock Device], device 0: Mock [Mock]"
"#,
        );
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));
        let pass = check_microphone_permission(true);
        assert_eq!(pass.status, CheckStatus::Pass);

        write_script(&bin.join("arecord"), "#!/bin/sh\necho \"\"\n");
        let warn = check_microphone_permission(true);
        assert_eq!(warn.status, CheckStatus::Warn);

        write_script(
            &bin.join("arecord"),
            "#!/bin/sh\nexit 1\n",
        );
        let warn_error = check_microphone_permission(true);
        assert_eq!(warn_error.status, CheckStatus::Warn);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn recording_backend_capability_outcomes() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
echo "card 0: Device [Mock Device], device 0: Mock [Mock]"
"#,
        );
        let pass = check_recording_backend_capability();
        assert_eq!(pass.status, CheckStatus::Pass);

        write_script(&bin.join("arecord"), "#!/bin/sh\necho \"\"\n");
        let warn = check_recording_backend_capability();
        assert_eq!(warn.status, CheckStatus::Warn);

        fs::remove_file(bin.join("arecord")).expect("remove");
        write_script(&bin.join("ffmpeg"), "#!/bin/sh\necho \"ffmpeg version 8.0\"\n");
        let pass_ffmpeg = check_recording_backend_capability();
        assert_eq!(pass_ffmpeg.status, CheckStatus::Pass);

        fs::remove_file(bin.join("ffmpeg")).expect("remove ffmpeg");
        let fail = check_recording_backend_capability();
        assert_eq!(fail.status, CheckStatus::Fail);
    }
}

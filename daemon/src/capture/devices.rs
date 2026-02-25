#[cfg(target_os = "macos")]
use cpal::traits::{DeviceTrait, HostTrait};

use crate::error::{AppError, AppResult};

#[cfg(target_os = "macos")]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|error| {
        AppError::Capture(format!("failed to enumerate input devices: {error}"))
    })?;

    let mut names = Vec::new();
    for device in devices {
        let name = device
            .name()
            .map_err(|error| AppError::Capture(format!("failed to read device name: {error}")))?;
        names.push(name);
    }

    Ok(names)
}

#[cfg(target_os = "linux")]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    if which::which("arecord").is_ok() {
        let output = std::process::Command::new("arecord")
            .arg("-l")
            .output()
            .map_err(|error| {
                AppError::Capture(format!("failed to execute `arecord -l`: {error}"))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = stdout
            .lines()
            .filter(|line| line.contains("card "))
            .map(|line| line.trim().to_owned())
            .collect::<Vec<_>>();

        return Ok(devices);
    }

    if which::which("ffmpeg").is_ok() {
        return Ok(vec!["default (ffmpeg/alsa input)".to_owned()]);
    }

    Err(AppError::BinaryMissing {
        binary: "arecord or ffmpeg".to_owned(),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    Err(AppError::Capture(
        "input device enumeration is only implemented for macOS and Linux in v1".to_owned(),
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::list_input_devices;
    use crate::error::AppError;
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
        fs::write(path, body).expect("write");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }

    #[test]
    fn list_devices_prefers_arecord() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));
        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
echo "card 0: Mock [Mock], device 0: USB [USB]"
"#,
        );
        write_script(
            &bin.join("ffmpeg"),
            r#"#!/bin/sh
echo "ffmpeg version 9.0"
"#,
        );
        let devices = list_input_devices().expect("devices");
        assert_eq!(devices.len(), 1);
        assert!(devices[0].contains("card 0"));
    }

    #[test]
    fn list_devices_falls_back_to_ffmpeg() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));
        write_script(
            &bin.join("ffmpeg"),
            r#"#!/bin/sh
echo "ffmpeg version 9.0"
"#,
        );
        let devices = list_input_devices().expect("devices");
        assert_eq!(devices, vec!["default (ffmpeg/alsa input)".to_owned()]);
    }

    #[test]
    fn list_devices_errors_when_no_recorders() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        let err = list_input_devices().expect_err("must fail");
        assert!(matches!(err, AppError::BinaryMissing { .. }));
    }
}

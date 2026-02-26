use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use franken_whisper::storage::RunStore;
use franken_whisper::BackendKind;
use fsqlite_types::value::SqliteValue;
use quedo_daemon::bootstrap::{bootstrap_env, AppPaths};
use quedo_daemon::config::{AppConfig, OutputMode, TranscriptionConfig};
use quedo_daemon::controller::events::{ControllerEvent, ControllerOutput};
use quedo_daemon::controller::state::ControllerState;
use quedo_daemon::controller::{run_controller_loop, ControllerContext};
use serde_json::Value;
use tempfile::TempDir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn acquire_env_lock() -> std::sync::MutexGuard<'static, ()> {
    env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: String) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: test code mutates process env and restores it in Drop.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            // SAFETY: restoring env var to previous value.
            unsafe {
                std::env::set_var(self.key, previous);
            }
        } else {
            // SAFETY: restoring env var absence.
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }
}

fn fixture_candidates() -> [PathBuf; 2] {
    [
        PathBuf::from("/home/jorge/.local/src/whisper.cpp/samples/jfk.wav"),
        PathBuf::from("/tmp/franken_whisper/test_data/jfk.wav"),
    ]
}

fn resolve_fixture_wav() -> PathBuf {
    fixture_candidates()
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("fixture missing at expected paths"))
}

fn resolve_model_path() -> PathBuf {
    let candidate = PathBuf::from("/home/jorge/.local/share/quedo/models/ggml-base.en.bin");
    assert!(
        candidate.exists(),
        "model missing at {}",
        candidate.display()
    );
    candidate
}

fn normalize_text(raw: &str) -> String {
    raw.to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '\'' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let reference_words: Vec<&str> = reference.split_whitespace().collect();
    let hypothesis_words: Vec<&str> = hypothesis.split_whitespace().collect();

    if reference_words.is_empty() {
        return if hypothesis_words.is_empty() {
            0.0
        } else {
            1.0
        };
    }

    let rows = reference_words.len() + 1;
    let cols = hypothesis_words.len() + 1;
    let mut dp = vec![vec![0_usize; cols]; rows];

    for (row, item) in dp.iter_mut().enumerate() {
        item[0] = row;
    }
    for (col, item) in dp[0].iter_mut().enumerate() {
        *item = col;
    }

    for row in 1..rows {
        for col in 1..cols {
            let substitution_cost = if reference_words[row - 1] == hypothesis_words[col - 1] {
                0
            } else {
                1
            };
            let substitution = dp[row - 1][col - 1] + substitution_cost;
            let deletion = dp[row - 1][col] + 1;
            let insertion = dp[row][col - 1] + 1;
            dp[row][col] = substitution.min(deletion).min(insertion);
        }
    }

    dp[rows - 1][cols - 1] as f64 / reference_words.len() as f64
}

fn make_paths(root: &Path) -> AppPaths {
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

fn spawn_controller(
    context: ControllerContext,
) -> (
    crossbeam_channel::Sender<ControllerEvent>,
    crossbeam_channel::Receiver<ControllerOutput>,
    std::thread::JoinHandle<quedo_daemon::error::AppResult<()>>,
) {
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<ControllerEvent>();
    let (output_tx, output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();
    let loop_event_tx = event_tx.clone();
    let join = std::thread::spawn(move || {
        run_controller_loop(context, event_rx, loop_event_tx, output_tx)
    });
    (event_tx, output_rx, join)
}

fn recv_until<F>(
    output_rx: &crossbeam_channel::Receiver<ControllerOutput>,
    timeout: Duration,
    predicate: F,
) -> ControllerOutput
where
    F: Fn(&ControllerOutput) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let now = Instant::now();
        let remaining = deadline.saturating_duration_since(now);
        let output = output_rx.recv_timeout(remaining).unwrap_or_else(|_| {
            panic!("timed out waiting for controller output after {timeout:?}")
        });
        if predicate(&output) {
            return output;
        }
    }
}

fn write_script(script_path: &Path, script: &str) {
    fs::write(script_path, script).unwrap_or_else(|error| {
        panic!("failed to write wrapper {}: {error}", script_path.display())
    });

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(script_path)
            .unwrap_or_else(|error| panic!("metadata {}: {error}", script_path.display()))
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(script_path, permissions)
            .unwrap_or_else(|error| panic!("chmod {}: {error}", script_path.display()));
    }
}

fn write_wrapper(dir: &Path, name: &str, target: &Path) {
    let script_path = dir.join(name);
    let script = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", target.display());
    write_script(&script_path, &script);
}

fn write_whisper_cli_wrapper(dir: &Path, target: &Path) {
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n  --version|-V|version)\n    echo \"whisper-cli 1.7.2\"\n    exit 0\n    ;;\nesac\nexec \"{}\" \"$@\"\n",
        target.display()
    );
    write_script(&dir.join("whisper-cli"), &script);
}

fn write_arecord_fixture_wrapper(dir: &Path, fixture: &Path) {
    let script = format!(
        "#!/bin/sh\nout=\"\"\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"-l\" ]; then\n    echo \"card 0: Mock [Mock], device 0: USB [USB]\"\n    exit 0\n  fi\n  out=\"$arg\"\ndone\ncp \"{}\" \"$out\"\nsleep 30\n",
        fixture.display()
    );
    write_script(&dir.join("arecord"), &script);
}

fn write_arecord_sequence_wrapper(
    dir: &Path,
    empty_wav: &Path,
    corrupt_wav: &Path,
    valid_wav: &Path,
    counter_file: &Path,
) {
    let script = format!(
        "#!/bin/sh\nout=\"\"\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"-l\" ]; then\n    echo \"card 0: Mock [Mock], device 0: USB [USB]\"\n    exit 0\n  fi\n  out=\"$arg\"\ndone\ncount=0\nif [ -f \"{counter}\" ]; then count=$(cat \"{counter}\"); fi\nif [ \"$count\" -eq 0 ]; then src=\"{empty}\";\nelif [ \"$count\" -eq 1 ]; then src=\"{corrupt}\";\nelse src=\"{valid}\";\nfi\ncp \"$src\" \"$out\"\necho $((count+1)) > \"{counter}\"\nsleep 30\n",
        counter = counter_file.display(),
        empty = empty_wav.display(),
        corrupt = corrupt_wav.display(),
        valid = valid_wav.display()
    );
    write_script(&dir.join("arecord"), &script);
}

fn run_whisper_cli_to_text(
    wav_path: &Path,
    model_path: &Path,
    output_prefix: &Path,
    path_override: &str,
) -> String {
    let status = Command::new("whisper-cli")
        .env("PATH", path_override)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(wav_path)
        .arg("-l")
        .arg("en")
        .arg("-otxt")
        .arg("-of")
        .arg(output_prefix)
        .status()
        .expect("run whisper-cli");

    assert!(status.success(), "whisper-cli exited with {status}");

    let txt_path = output_prefix.with_extension("txt");
    fs::read_to_string(&txt_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", txt_path.display()))
}

fn build_default_config(model: &Path) -> AppConfig {
    let mut config = AppConfig::default();
    config.output.mode = OutputMode::Disabled;
    config.audio.retain_audio = true;
    config.transcription = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        threads: Some(2),
        processors: Some(1),
        ..TranscriptionConfig::default()
    };
    config
}

#[test]
fn metal_backend_requires_structured_evidence() {
    if !cfg!(target_os = "macos") || !cfg!(target_arch = "aarch64") {
        eprintln!("SKIPPED: Metal structured evidence check requires macOS ARM64");
        return;
    }

    let _lock = acquire_env_lock();
    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("mkdir wrappers");

    write_arecord_fixture_wrapper(&wrappers, &fixture);
    write_wrapper(
        &wrappers,
        "ffmpeg",
        &which::which("ffmpeg").expect("ffmpeg path"),
    );
    write_wrapper(
        &wrappers,
        "ffprobe",
        &which::which("ffprobe").expect("ffprobe path"),
    );
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let _path_guard = EnvVarGuard::set(
        "PATH",
        format!(
            "{}:{}",
            wrappers.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");
    let context = ControllerContext {
        config: build_default_config(&model),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });
    event_tx
        .send(ControllerEvent::Toggle)
        .expect("toggle start");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Recording)
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Processing)
        )
    });

    let transcript = match recv_until(&output_rx, Duration::from_secs(120), |output| {
        matches!(output, ControllerOutput::TranscriptReady(_))
    }) {
        ControllerOutput::TranscriptReady(result) => result,
        other => panic!("expected transcript output, got {other:?}"),
    };

    let store = RunStore::open(&paths.history_db).expect("open run store");
    let details = store
        .load_run_details(&transcript.run_id)
        .expect("load run details")
        .expect("run details");
    let backend_ok = details
        .events
        .iter()
        .find(|event| event.code == "backend.ok")
        .expect("backend.ok event");

    assert!(
        backend_ok
            .payload
            .get("resolved_backend")
            .and_then(Value::as_str)
            .is_some(),
        "structured backend identity missing"
    );
    assert!(
        backend_ok.payload.get("execution_mode").is_some(),
        "structured execution mode missing"
    );
    assert!(
        backend_ok.payload.get("native_rollout_stage").is_some(),
        "structured rollout stage missing"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
fn capture_artifact_is_native_before_normalize() {
    let _lock = acquire_env_lock();
    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");

    let native_capture = temp.path().join("native_capture.wav");
    let convert = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&fixture)
        .args(["-ac", "2", "-ar", "48000", "-c:a", "pcm_s16le"])
        .arg(&native_capture)
        .status()
        .expect("ffmpeg convert fixture to native capture");
    assert!(convert.success(), "ffmpeg conversion failed: {convert}");

    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("mkdir wrappers");
    write_arecord_fixture_wrapper(&wrappers, &native_capture);
    write_wrapper(
        &wrappers,
        "ffmpeg",
        &which::which("ffmpeg").expect("ffmpeg path"),
    );
    write_wrapper(
        &wrappers,
        "ffprobe",
        &which::which("ffprobe").expect("ffprobe path"),
    );
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let _path_guard = EnvVarGuard::set(
        "PATH",
        format!(
            "{}:{}",
            wrappers.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");
    let context = ControllerContext {
        config: build_default_config(&model),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });
    event_tx
        .send(ControllerEvent::Toggle)
        .expect("toggle start");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Recording)
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Processing)
        )
    });

    let transcript = match recv_until(&output_rx, Duration::from_secs(120), |output| {
        matches!(output, ControllerOutput::TranscriptReady(_))
    }) {
        ControllerOutput::TranscriptReady(result) => result,
        other => panic!("expected transcript output, got {other:?}"),
    };

    let store = RunStore::open(&paths.history_db).expect("open run store");
    let details = store
        .load_run_details(&transcript.run_id)
        .expect("load run details")
        .expect("run details");

    let ingest_ok = details
        .events
        .iter()
        .find(|event| event.code == "ingest.ok")
        .expect("ingest.ok event");
    let normalize_ok = details
        .events
        .iter()
        .find(|event| event.code == "normalize.ok")
        .expect("normalize.ok event");
    let backend_ok = details
        .events
        .iter()
        .find(|event| event.code == "backend.ok")
        .expect("backend.ok event");

    let ingest_path = ingest_ok
        .payload
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .expect("ingest path");
    let normalized_path = normalize_ok
        .payload
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .expect("normalized path");

    assert_ne!(
        ingest_path, normalized_path,
        "capture path must differ from normalized output path"
    );

    let capture_reader = hound::WavReader::open(&ingest_path).expect("open capture wav");
    let capture_spec = capture_reader.spec();
    assert!(
        capture_spec.channels != 1 || capture_spec.sample_rate != 16_000,
        "capture stage appears canonicalized too early: channels={}, rate={}",
        capture_spec.channels,
        capture_spec.sample_rate
    );

    let normalized_name = normalized_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        normalized_name.contains("normalized"),
        "normalize stage should write a normalized artifact path, got {}",
        normalized_path.display()
    );
    assert!(
        normalize_ok
            .payload
            .get("duration_seconds")
            .and_then(Value::as_f64)
            .is_some(),
        "normalize stage should emit duration metadata"
    );

    let ingest_index = details
        .events
        .iter()
        .position(|event| event.code == "ingest.ok")
        .expect("ingest position");
    let normalize_index = details
        .events
        .iter()
        .position(|event| event.code == "normalize.ok")
        .expect("normalize position");
    let backend_index = details
        .events
        .iter()
        .position(|event| event.code == "backend.ok")
        .expect("backend position");
    assert!(
        ingest_index < normalize_index && normalize_index < backend_index,
        "pipeline stage order must be capture -> normalize -> backend"
    );
    assert!(
        backend_ok
            .payload
            .get("resolved_backend")
            .and_then(Value::as_str)
            .is_some(),
        "backend completion payload must be present"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
fn forbidden_runtime_modes_have_no_listener_behavior() {
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("SKIPPED: /proc fd listener scan is only implemented for Linux CI");
        return;
    }

    #[cfg(target_os = "linux")]
    {
        let temp = TempDir::new().expect("tempdir");
        let mut child = Command::new(env!("CARGO_BIN_EXE_quedo-daemon"))
            .arg("run")
            .env("HOME", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("xdg-config"))
            .env("XDG_DATA_HOME", temp.path().join("xdg-data"))
            .env("XDG_CACHE_HOME", temp.path().join("xdg-cache"))
            .spawn()
            .expect("spawn daemon run");

        std::thread::sleep(Duration::from_secs(2));

        let fd_dir = PathBuf::from(format!("/proc/{}/fd", child.id()));
        let entries = fs::read_dir(&fd_dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", fd_dir.display()));
        for entry in entries {
            let entry = entry.expect("fd entry");
            let target = fs::read_link(entry.path()).unwrap_or_else(|_| PathBuf::new());
            let target_text = target.display().to_string();
            assert!(
                !target_text.contains("socket:"),
                "forbidden listener/socket fd detected: {}",
                target_text
            );
        }

        let _ = child.kill();
        let _ = child.wait();
    }
}

#[test]
fn persisted_request_metadata_contains_contract_fields() {
    let _lock = acquire_env_lock();
    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("mkdir wrappers");

    write_arecord_fixture_wrapper(&wrappers, &fixture);
    write_wrapper(
        &wrappers,
        "ffmpeg",
        &which::which("ffmpeg").expect("ffmpeg path"),
    );
    write_wrapper(
        &wrappers,
        "ffprobe",
        &which::which("ffprobe").expect("ffprobe path"),
    );
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let _path_guard = EnvVarGuard::set(
        "PATH",
        format!(
            "{}:{}",
            wrappers.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");

    let mut config = build_default_config(&model);
    config.transcription.diarize = true;
    config.transcription.timeout_seconds = 95;
    config.transcription.threads = Some(5);
    config.transcription.processors = Some(2);

    let context = ControllerContext {
        config: config.clone(),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });
    event_tx
        .send(ControllerEvent::Toggle)
        .expect("toggle start");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Recording)
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Processing)
        )
    });

    let transcript = match recv_until(&output_rx, Duration::from_secs(120), |output| {
        matches!(output, ControllerOutput::TranscriptReady(_))
    }) {
        ControllerOutput::TranscriptReady(result) => result,
        other => panic!("expected transcript output, got {other:?}"),
    };

    let store = RunStore::open(&paths.history_db).expect("open run store");
    let session = store
        .begin_concurrent_session("request_metadata_read")
        .expect("begin read session");
    let sql = format!(
        "SELECT request_json FROM runs WHERE id = '{}'",
        transcript.run_id.replace('\'', "''")
    );
    let rows = session.query(&sql).expect("query request_json");
    let row = rows.first().expect("request_json row");
    let request_json = match row.get(0) {
        Some(SqliteValue::Text(value)) => value.clone(),
        other => panic!("unexpected request_json column type: {other:?}"),
    };
    let request: Value = serde_json::from_str(&request_json).expect("parse request json");

    assert_eq!(request.get("persist").and_then(Value::as_bool), Some(true));
    assert_eq!(
        request.get("diarize").and_then(Value::as_bool),
        Some(config.transcription.diarize)
    );
    assert_eq!(
        request.get("timeout_ms").and_then(Value::as_u64),
        Some(config.transcription.timeout_ms())
    );
    assert_eq!(
        request.get("db_path").and_then(Value::as_str),
        Some(paths.history_db.to_string_lossy().as_ref())
    );
    assert_eq!(
        request
            .get("backend_params")
            .and_then(|bp| bp.get("threads"))
            .and_then(Value::as_u64),
        Some(config.transcription.threads.expect("threads") as u64)
    );
    assert_eq!(
        request
            .get("backend_params")
            .and_then(|bp| bp.get("processors"))
            .and_then(Value::as_u64),
        Some(config.transcription.processors.expect("processors") as u64)
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
fn missing_ffmpeg_disables_recording_in_unavailable_mode_release_gate() {
    let _lock = acquire_env_lock();

    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("create wrappers dir");

    write_arecord_fixture_wrapper(&wrappers, &fixture);
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );
    let _path_guard = EnvVarGuard::set("PATH", wrappers.display().to_string());

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");

    let context = ControllerContext {
        config: build_default_config(&model),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let startup_state = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });
    let unavailable_reason = match startup_state {
        ControllerOutput::StateChanged(ControllerState::Unavailable(reason)) => reason,
        other => panic!("expected unavailable startup state, got {other:?}"),
    };
    assert!(
        unavailable_reason.to_ascii_lowercase().contains("ffmpeg")
            || unavailable_reason.to_ascii_lowercase().contains("ffprobe"),
        "unexpected unavailable reason: {unavailable_reason}"
    );

    event_tx.send(ControllerEvent::Toggle).expect("toggle");
    let blocked_note = match recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Notification(_))
    }) {
        ControllerOutput::Notification(message) => message,
        other => panic!("expected notification, got {other:?}"),
    };
    assert!(
        blocked_note
            .to_ascii_lowercase()
            .contains("recording disabled"),
        "unexpected unavailable notification: {blocked_note}"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
fn corrupt_and_empty_wav_fail_gracefully_release_gate() {
    let _lock = acquire_env_lock();
    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");

    let empty_wav = temp.path().join("empty_header_only.wav");
    let writer = hound::WavWriter::create(
        &empty_wav,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .expect("create empty wav");
    writer.finalize().expect("finalize empty wav");

    let corrupt_wav = temp.path().join("corrupt_random_bytes.wav");
    fs::write(
        &corrupt_wav,
        [0x13_u8, 0x37, 0x42, 0x00, 0x99, 0xEF, 0xAA, 0xBB],
    )
    .expect("write corrupt wav");

    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("create wrappers dir");
    write_arecord_sequence_wrapper(
        &wrappers,
        &empty_wav,
        &corrupt_wav,
        &fixture,
        &temp.path().join("arecord-count"),
    );
    write_wrapper(
        &wrappers,
        "ffmpeg",
        &which::which("ffmpeg").expect("ffmpeg path"),
    );
    write_wrapper(
        &wrappers,
        "ffprobe",
        &which::which("ffprobe").expect("ffprobe path"),
    );
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let _path_guard = EnvVarGuard::set(
        "PATH",
        format!(
            "{}:{}",
            wrappers.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");
    let context = ControllerContext {
        config: build_default_config(&model),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });

    let mut degraded_reasons = Vec::new();
    for _ in 0..2 {
        event_tx
            .send(ControllerEvent::Toggle)
            .expect("toggle start");
        let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Recording)
            )
        });
        std::thread::sleep(Duration::from_millis(250));
        event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
        let next_state = recv_until(&output_rx, Duration::from_secs(60), |output| {
            matches!(output, ControllerOutput::StateChanged(_))
        });
        let degraded = match next_state {
            ControllerOutput::StateChanged(ControllerState::Degraded(reason)) => reason,
            ControllerOutput::StateChanged(ControllerState::Processing) => {
                match recv_until(&output_rx, Duration::from_secs(60), |output| {
                    matches!(
                        output,
                        ControllerOutput::StateChanged(ControllerState::Degraded(_))
                    )
                }) {
                    ControllerOutput::StateChanged(ControllerState::Degraded(reason)) => reason,
                    other => panic!("expected degraded state, got {other:?}"),
                }
            }
            other => panic!("expected degraded or processing state, got {other:?}"),
        };
        degraded_reasons.push(degraded);
    }

    assert_eq!(degraded_reasons.len(), 2);
    assert_ne!(
        degraded_reasons[0], degraded_reasons[1],
        "empty and corrupt WAV failures should produce distinct details"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
fn differential_reference_comparison_matches_whisper_cli() {
    let _lock = acquire_env_lock();
    let fixture = resolve_fixture_wav();
    let model = resolve_model_path();
    let temp = TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("mkdir wrappers");

    write_arecord_fixture_wrapper(&wrappers, &fixture);
    write_wrapper(
        &wrappers,
        "ffmpeg",
        &which::which("ffmpeg").expect("ffmpeg path"),
    );
    write_wrapper(
        &wrappers,
        "ffprobe",
        &which::which("ffprobe").expect("ffprobe path"),
    );
    write_whisper_cli_wrapper(
        &wrappers,
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let path_override = format!(
        "{}:{}",
        wrappers.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let _path_guard = EnvVarGuard::set("PATH", path_override.clone());

    let paths = make_paths(temp.path());
    paths.ensure_dirs().expect("ensure dirs");
    bootstrap_env(&paths).expect("bootstrap env");
    let context = ControllerContext {
        config: build_default_config(&model),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::StateChanged(_))
    });
    event_tx
        .send(ControllerEvent::Toggle)
        .expect("toggle start");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Recording)
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Processing)
        )
    });

    let transcript = match recv_until(&output_rx, Duration::from_secs(120), |output| {
        matches!(output, ControllerOutput::TranscriptReady(_))
    }) {
        ControllerOutput::TranscriptReady(result) => result,
        other => panic!("expected transcript output, got {other:?}"),
    };

    let whisper_output_prefix = temp.path().join("jfk_direct_cli");
    let direct = run_whisper_cli_to_text(&fixture, &model, &whisper_output_prefix, &path_override);
    let reference =
        "And so my fellow Americans ask not what your country can do for you ask what you can do for your country";

    let daemon_wer = word_error_rate(
        &normalize_text(reference),
        &normalize_text(&transcript.transcript),
    );
    let direct_wer = word_error_rate(&normalize_text(reference), &normalize_text(&direct));
    assert!(
        daemon_wer <= direct_wer + 0.05,
        "daemon WER drift too high vs direct whisper-cli: daemon={daemon_wer:.4}, direct={direct_wer:.4}"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use franken_whisper::BackendKind;
use quedo_daemon::bootstrap::{bootstrap_env, AppPaths};
use quedo_daemon::config::{AppConfig, OutputMode, TranscriptionConfig};
use quedo_daemon::controller::events::{ControllerEvent, ControllerOutput};
use quedo_daemon::controller::queue::SingleFlightQueue;
use quedo_daemon::controller::state::ControllerState;
use quedo_daemon::controller::{run_controller_loop, ControllerContext};
use quedo_daemon::history::HistoryStore;
use rusqlite::Connection;
use serde_json::Value;

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
        // SAFETY: test code intentionally mutates process env and restores it via Drop.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            // SAFETY: restoring previous process env value in test teardown.
            unsafe {
                std::env::set_var(self.key, previous);
            }
        } else {
            // SAFETY: restoring unset process env value in test teardown.
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }
}

fn local_bin_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/jorge".to_owned());
    PathBuf::from(home).join(".local/bin")
}

fn path_with_local_bin() -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", local_bin_dir().display(), current)
}

fn fixture_candidates() -> [PathBuf; 2] {
    [
        PathBuf::from("/tmp/franken_whisper/test_data/jfk.wav"),
        PathBuf::from("/home/jorge/.local/src/whisper.cpp/samples/jfk.wav"),
    ]
}

fn resolve_fixture_wav() -> Option<PathBuf> {
    fixture_candidates().into_iter().find(|path| path.exists())
}

fn resolve_model_path() -> Option<PathBuf> {
    let candidate = PathBuf::from("/home/jorge/.local/share/quedo/models/ggml-base.en.bin");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn resolve_jiwer_python() -> Option<PathBuf> {
    let venv_python = PathBuf::from("/home/jorge/.local/share/quedo/venvs/wer/bin/python");
    if venv_python.exists() {
        return Some(venv_python);
    }

    which::which("python3").ok()
}

fn tool_available(tool: &str) -> bool {
    which::which(tool).is_ok()
}

fn should_skip(required_tools: &[&str], requires_fixture: bool, requires_model: bool) -> bool {
    let missing_tools: Vec<&str> = required_tools
        .iter()
        .copied()
        .filter(|tool| !tool_available(tool))
        .collect();

    if !missing_tools.is_empty() {
        eprintln!(
            "SKIPPED: missing required tools: {}",
            missing_tools.join(", ")
        );
        return true;
    }

    if requires_fixture && resolve_fixture_wav().is_none() {
        eprintln!(
            "SKIPPED: fixture jfk.wav missing at {:?}",
            fixture_candidates()
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
        );
        return true;
    }

    if requires_model && resolve_model_path().is_none() {
        eprintln!(
            "SKIPPED: model missing at /home/jorge/.local/share/quedo/models/ggml-base.en.bin"
        );
        return true;
    }

    false
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

#[test]
fn sqlite_history_roundtrip_with_real_sqlite() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp.path().join("history.sqlite3");

    let conn = Connection::open(&db_path).expect("open sqlite");
    conn.execute_batch(
        "CREATE TABLE runs (
            id TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT NOT NULL,
            backend TEXT NOT NULL,
            transcript TEXT NOT NULL
        );",
    )
    .expect("create schema");

    conn.execute(
        "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            "run-old",
            "2026-02-25T00:00:00Z",
            "2026-02-25T00:00:01Z",
            "whisper_cpp",
            "old transcript",
        ),
    )
    .expect("insert old");

    conn.execute(
        "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            "run-new",
            "2026-02-25T00:10:00Z",
            "2026-02-25T00:10:01Z",
            "insanely_fast",
            "new transcript",
        ),
    )
    .expect("insert new");

    let history = HistoryStore::new(db_path);
    let runs = history.list_recent_runs(10).expect("list runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].run_id, "run-new");
    assert_eq!(runs[1].run_id, "run-old");

    let latest = history.latest_run().expect("latest").expect("present");
    assert_eq!(latest.run_id, "run-new");
}

#[test]
fn rapid_queue_operations_hold_single_flight_policy() {
    let mut queue = SingleFlightQueue::new(1);

    for index in 0..2_000 {
        let first = PathBuf::from(format!("/tmp/job-{index}.wav"));
        queue.enqueue(first.clone()).expect("first enqueue");

        let overflow = queue.enqueue(PathBuf::from(format!("/tmp/job-{index}-overflow.wav")));
        assert!(
            overflow.is_err(),
            "queue accepted an overflow job at index {index}"
        );

        assert_eq!(queue.start_next(), Some(first));
        assert!(queue.start_next().is_none());

        queue.mark_finished();
        queue.mark_finished();
    }
}

#[test]
#[ignore = "requires local ffmpeg/ffprobe/whisper-cli model + fixture"]
fn doctor_command_json_output_matches_schema_with_real_binaries() {
    if should_skip(&["ffmpeg", "ffprobe", "whisper-cli"], false, false) {
        return;
    }

    let temp = tempfile::TempDir::new().expect("tempdir");
    let path = path_with_local_bin();

    let output = Command::new(env!("CARGO_BIN_EXE_quedo-daemon"))
        .arg("doctor")
        .arg("--json")
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("xdg-config"))
        .env("XDG_DATA_HOME", temp.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", temp.path().join("xdg-cache"))
        .output()
        .expect("run doctor --json");

    assert!(
        output.status.success(),
        "doctor --json failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
    assert!(json
        .get("generated_at_rfc3339")
        .and_then(Value::as_str)
        .is_some());
    assert!(
        matches!(
            json.get("state").and_then(Value::as_str),
            Some("ready" | "degraded" | "unavailable")
        ),
        "unexpected doctor state: {:?}",
        json.get("state")
    );

    let checks = json
        .get("checks")
        .and_then(Value::as_array)
        .expect("checks array");
    assert!(!checks.is_empty(), "doctor checks must not be empty");

    for check in checks {
        assert!(check.get("name").and_then(Value::as_str).is_some());
        assert!(
            matches!(
                check.get("status").and_then(Value::as_str),
                Some("pass" | "warn" | "fail" | "skip")
            ),
            "unexpected check status: {check:?}"
        );
        assert!(check.get("detail").and_then(Value::as_str).is_some());
        assert!(check.get("required").and_then(Value::as_bool).is_some());
    }

    for required_name in ["ffmpeg", "ffprobe", "whisper-cli"] {
        let check = checks
            .iter()
            .find(|check| check.get("name").and_then(Value::as_str) == Some(required_name))
            .unwrap_or_else(|| panic!("required doctor check `{required_name}` is missing"));
        assert_eq!(
            check.get("required").and_then(Value::as_bool),
            Some(true),
            "check `{required_name}` should be required"
        );
        assert!(
            matches!(
                check.get("status").and_then(Value::as_str),
                Some("pass" | "warn" | "fail")
            ),
            "required check `{required_name}` must not be skipped: {check:?}"
        );
    }
}

#[test]
#[ignore = "requires local ffmpeg + fixture"]
fn real_ffmpeg_normalize_processes_wav_fixture() {
    if should_skip(&["ffmpeg"], true, false) {
        return;
    }

    let fixture = resolve_fixture_wav().expect("fixture");
    let temp = tempfile::TempDir::new().expect("tempdir");
    let normalized = temp.path().join("normalized.wav");

    let status = Command::new("ffmpeg")
        .env("PATH", path_with_local_bin())
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&fixture)
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(&normalized)
        .status()
        .expect("run ffmpeg normalize");

    assert!(status.success(), "ffmpeg normalize failed with {status}");
    assert!(normalized.exists());

    let reader = hound::WavReader::open(&normalized).expect("open normalized wav");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 16_000);
    assert_eq!(spec.bits_per_sample, 16);
    assert!(reader.duration() > 0, "normalized WAV should not be empty");
}

#[test]
#[ignore = "requires local whisper-cli model + fixture"]
fn real_whisper_cli_transcription_is_non_empty() {
    if should_skip(&["whisper-cli"], true, true) {
        return;
    }

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");
    let output_prefix = temp.path().join("jfk_transcript");

    let transcript =
        run_whisper_cli_to_text(&fixture, &model, &output_prefix, &path_with_local_bin());
    let reference =
        "And so my fellow Americans ask not what your country can do for you ask what you can do for your country";
    let wer = word_error_rate(&normalize_text(reference), &normalize_text(&transcript));
    assert!(
        wer <= 0.35,
        "whisper-cli transcript WER too high: {wer:.4}, transcript={transcript:?}"
    );
}

#[test]
#[ignore = "requires whisper-cli + jiwer venv + model + fixture"]
fn wer_scoring_with_jiwer_is_within_reasonable_threshold() {
    if should_skip(&["whisper-cli"], true, true) {
        return;
    }

    let python = match resolve_jiwer_python() {
        Some(path) => path,
        None => {
            eprintln!("SKIPPED: python interpreter not found");
            return;
        }
    };

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");
    let output_prefix = temp.path().join("jfk_wer");

    let transcript =
        run_whisper_cli_to_text(&fixture, &model, &output_prefix, &path_with_local_bin());

    let reference =
        "And so my fellow Americans ask not what your country can do for you ask what you can do for your country.";
    let script = "import jiwer,re,sys; norm=lambda s:' '.join(re.findall(r\"[a-z0-9']+\", s.lower())); print(jiwer.wer(norm(sys.argv[1]), norm(sys.argv[2])))";

    let output = Command::new(&python)
        .arg("-c")
        .arg(script)
        .arg(reference)
        .arg(&transcript)
        .output()
        .expect("run jiwer");

    assert!(
        output.status.success(),
        "jiwer failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let wer: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("parse wer float");

    assert!(wer <= 0.35, "WER too high: {wer}");
}

#[test]
#[ignore = "requires full local ffmpeg + whisper-cli + model + fixture"]
fn full_pipeline_e2e_fixture_to_transcript_and_history() {
    if should_skip(&["ffmpeg", "whisper-cli"], true, true) {
        return;
    }

    let _lock = acquire_env_lock();
    let _path_guard = EnvVarGuard::set("PATH", path_with_local_bin());

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");
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
    write_wrapper(
        &wrappers,
        "whisper-cli",
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

    let mut config = AppConfig::default();
    config.output.mode = OutputMode::Disabled;
    config.audio.retain_audio = true;
    config.transcription = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };

    let context = ControllerContext {
        config: config.clone(),
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    assert!(matches!(
        recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Idle)
            )
        }),
        ControllerOutput::StateChanged(ControllerState::Idle)
    ));

    event_tx
        .send(ControllerEvent::Toggle)
        .expect("toggle start");
    assert!(matches!(
        recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Recording)
            )
        }),
        ControllerOutput::StateChanged(ControllerState::Recording)
    ));
    std::thread::sleep(Duration::from_millis(250));

    event_tx.send(ControllerEvent::Toggle).expect("toggle stop");
    assert!(matches!(
        recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Processing)
            )
        }),
        ControllerOutput::StateChanged(ControllerState::Processing)
    ));

    let transcript = match recv_until(&output_rx, Duration::from_secs(120), |output| {
        matches!(output, ControllerOutput::TranscriptReady(_))
    }) {
        ControllerOutput::TranscriptReady(result) => result,
        other => panic!("expected transcript output, got {other:?}"),
    };
    let normalized_transcript = normalize_text(&transcript.transcript);
    assert!(
        normalized_transcript.contains("ask not what your country can do for you"),
        "unexpected transcript: {}",
        transcript.transcript
    );

    assert!(matches!(
        recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Idle)
            )
        }),
        ControllerOutput::StateChanged(ControllerState::Idle)
    ));

    assert!(
        paths.history_db.exists(),
        "expected history database to be created at {}",
        paths.history_db.display()
    );
    let history_metadata = fs::metadata(&paths.history_db).expect("history db metadata");
    assert!(
        history_metadata.len() > 0,
        "history database should not be empty"
    );

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    assert!(matches!(
        recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(output, ControllerOutput::Stopped)
        }),
        ControllerOutput::Stopped
    ));
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
}

#[test]
#[ignore = "requires daemon binary and local ffmpeg/ffprobe/python3"]
fn degraded_mode_when_whisper_cli_missing() {
    if should_skip(&["ffmpeg", "ffprobe", "python3"], false, false) {
        return;
    }

    let temp = tempfile::TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("create wrappers dir");

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
    write_wrapper(
        &wrappers,
        "python3",
        &which::which("python3").expect("python path"),
    );
    if let Ok(path) = which::which("arecord") {
        write_wrapper(&wrappers, "arecord", &path);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_quedo-daemon"))
        .arg("doctor")
        .arg("--json")
        .env("PATH", wrappers.as_os_str())
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join("xdg-config"))
        .env("XDG_DATA_HOME", temp.path().join("xdg-data"))
        .env("XDG_CACHE_HOME", temp.path().join("xdg-cache"))
        .output()
        .expect("run doctor --json");

    assert!(
        output.status.success(),
        "doctor --json failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("parse doctor json");
    let state = json
        .get("state")
        .and_then(Value::as_str)
        .expect("doctor state string");
    assert_ne!(
        state, "ready",
        "missing whisper-cli should not report ready"
    );

    let checks = json
        .get("checks")
        .and_then(Value::as_array)
        .expect("checks array");
    let whisper = checks
        .iter()
        .find(|check| check.get("name").and_then(Value::as_str) == Some("whisper-cli"))
        .expect("whisper check present");
    assert_eq!(whisper.get("status").and_then(Value::as_str), Some("fail"));
}

#[test]
#[ignore = "requires local whisper-cli + model + fixture"]
fn missing_ffmpeg_produces_graceful_transcription_error() {
    if should_skip(&["whisper-cli"], true, true) {
        return;
    }

    let _lock = acquire_env_lock();

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("create wrappers dir");

    write_arecord_fixture_wrapper(&wrappers, &fixture);
    write_wrapper(
        &wrappers,
        "whisper-cli",
        &which::which("whisper-cli").expect("whisper-cli path"),
    );
    write_script(
        &wrappers.join("ffmpeg"),
        "#!/bin/sh\necho 'ffmpeg missing' >&2\nexit 127\n",
    );
    write_script(
        &wrappers.join("ffprobe"),
        "#!/bin/sh\necho 'ffprobe missing' >&2\nexit 127\n",
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

    let mut config = AppConfig::default();
    config.output.mode = OutputMode::Disabled;
    config.audio.retain_audio = true;
    config.transcription = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };
    let context = ControllerContext {
        config,
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Idle)
        )
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

    let degraded_reason = match recv_until(&output_rx, Duration::from_secs(60), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Degraded(_))
        )
    }) {
        ControllerOutput::StateChanged(ControllerState::Degraded(reason)) => reason,
        other => panic!("expected degraded state, got {other:?}"),
    };
    assert!(
        degraded_reason.to_ascii_lowercase().contains("ffmpeg")
            || degraded_reason.to_ascii_lowercase().contains("normalize"),
        "unexpected degraded reason: {degraded_reason}"
    );

    let degraded_note = match recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Notification(_))
    }) {
        ControllerOutput::Notification(message) => message,
        other => panic!("expected notification, got {other:?}"),
    };
    assert!(
        degraded_note.to_ascii_lowercase().contains("ffmpeg")
            || degraded_note.to_ascii_lowercase().contains("normalize"),
        "unexpected degraded notification: {degraded_note}"
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
#[ignore = "requires local ffmpeg + whisper-cli + model"]
fn corrupt_and_empty_wav_fail_gracefully() {
    if should_skip(&["ffmpeg", "whisper-cli"], false, true) {
        return;
    }

    let _lock = acquire_env_lock();
    let _path_guard = EnvVarGuard::set("PATH", path_with_local_bin());

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");

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
    write_wrapper(
        &wrappers,
        "whisper-cli",
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

    let mut config = AppConfig::default();
    config.output.mode = OutputMode::Disabled;
    config.audio.retain_audio = true;
    config.transcription = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };
    let context = ControllerContext {
        config,
        paths: paths.clone(),
    };
    let (event_tx, output_rx, controller_join) = spawn_controller(context);

    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Idle)
        )
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
        let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Processing)
            )
        });
        let degraded = match recv_until(&output_rx, Duration::from_secs(60), |output| {
            matches!(
                output,
                ControllerOutput::StateChanged(ControllerState::Degraded(_))
            )
        }) {
            ControllerOutput::StateChanged(ControllerState::Degraded(reason)) => reason,
            other => panic!("expected degraded state, got {other:?}"),
        };
        degraded_reasons.push(degraded);
    }

    assert_eq!(degraded_reasons.len(), 2);
    assert_ne!(
        degraded_reasons[0], degraded_reasons[1],
        "empty and corrupt WAV failures should produce distinct details"
    );
    for reason in &degraded_reasons {
        assert!(
            reason.starts_with("transcription job failed:"),
            "unexpected degraded reason format: {reason}"
        );
    }

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
    assert!(
        normalize_text(&transcript.transcript).contains("ask not what your country can do for you"),
        "recovery transcript unexpected: {}",
        transcript.transcript
    );
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(
            output,
            ControllerOutput::StateChanged(ControllerState::Idle)
        )
    });

    event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
    let _ = recv_until(&output_rx, Duration::from_secs(5), |output| {
        matches!(output, ControllerOutput::Stopped)
    });
    controller_join
        .join()
        .expect("join controller")
        .expect("controller exit");
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

fn write_arecord_fixture_wrapper(dir: &Path, fixture: &Path) {
    let script = format!(
        "#!/bin/sh\nout=\"\"\nfor arg in \"$@\"; do out=\"$arg\"; done\ncp \"{}\" \"$out\"\nsleep 30\n",
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
        "#!/bin/sh\nout=\"\"\nfor arg in \"$@\"; do out=\"$arg\"; done\ncount=0\nif [ -f \"{counter}\" ]; then count=$(cat \"{counter}\"); fi\nif [ \"$count\" -eq 0 ]; then src=\"{empty}\";\nelif [ \"$count\" -eq 1 ]; then src=\"{corrupt}\";\nelse src=\"{valid}\";\nfi\ncp \"$src\" \"$out\"\necho $((count+1)) > \"{counter}\"\nsleep 30\n",
        counter = counter_file.display(),
        empty = empty_wav.display(),
        corrupt = corrupt_wav.display(),
        valid = valid_wav.display()
    );
    write_script(&dir.join("arecord"), &script);
}

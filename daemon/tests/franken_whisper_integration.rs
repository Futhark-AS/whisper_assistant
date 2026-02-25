use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use franken_whisper::BackendKind;
use quedo_daemon::config::TranscriptionConfig;
use quedo_daemon::controller::queue::SingleFlightQueue;
use quedo_daemon::error::AppError;
use quedo_daemon::history::HistoryStore;
use quedo_daemon::transcription::{run_transcription_job, FrankenEngine};
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
    assert!(
        !transcript.trim().is_empty(),
        "whisper-cli transcript should not be empty"
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
    let db_path = temp.path().join("history.sqlite3");

    let config = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };

    let engine = FrankenEngine::new().expect("engine init");
    let result = run_transcription_job(&engine, fixture, db_path.clone(), &config)
        .expect("full pipeline transcription");

    let normalized_transcript = normalize_text(&result.transcript);
    assert!(
        normalized_transcript.contains("ask not what your country can do for you"),
        "unexpected transcript: {}",
        result.transcript
    );

    assert!(db_path.exists(), "expected persistence file to be created");
    let metadata = fs::metadata(&db_path).expect("db metadata");
    assert!(metadata.len() > 0, "persistence file should not be empty");
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

    let temp = tempfile::TempDir::new().expect("tempdir");
    let wrappers = temp.path().join("bin");
    fs::create_dir_all(&wrappers).expect("create wrappers dir");
    write_wrapper(
        &wrappers,
        "whisper-cli",
        &which::which("whisper-cli").expect("whisper-cli path"),
    );

    let _path_guard = EnvVarGuard::set("PATH", wrappers.display().to_string());

    let fixture = resolve_fixture_wav().expect("fixture");
    let model = resolve_model_path().expect("model");
    let db_path = temp.path().join("history.sqlite3");

    let config = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };

    let engine = FrankenEngine::new().expect("engine init");
    let error = run_transcription_job(&engine, fixture, db_path, &config).expect_err("must fail");

    assert!(
        matches!(
            error,
            AppError::Transcription(ref message)
            if message.to_ascii_lowercase().contains("ffmpeg")
                || message.to_ascii_lowercase().contains("normalize")
        ),
        "unexpected error: {error}"
    );
}

#[test]
#[ignore = "requires local ffmpeg + whisper-cli + model"]
fn corrupt_and_empty_wav_fail_gracefully() {
    if should_skip(&["ffmpeg", "whisper-cli"], false, true) {
        return;
    }

    let _lock = acquire_env_lock();
    let _path_guard = EnvVarGuard::set("PATH", path_with_local_bin());

    let model = resolve_model_path().expect("model");
    let temp = tempfile::TempDir::new().expect("tempdir");

    let empty_wav = temp.path().join("empty.wav");
    fs::write(&empty_wav, []).expect("write empty wav");

    let corrupt_wav = temp.path().join("corrupt.wav");
    fs::write(
        &corrupt_wav,
        [0x13_u8, 0x37, 0x42, 0x00, 0x99, 0xEF, 0xAA, 0xBB],
    )
    .expect("write corrupt wav");

    let config = TranscriptionConfig {
        backend: BackendKind::WhisperCpp,
        model_id: Some(model.display().to_string()),
        language: Some("en".to_owned()),
        timeout_seconds: 120,
        ..TranscriptionConfig::default()
    };

    let engine = FrankenEngine::new().expect("engine init");

    for (input, db_name) in [
        (&empty_wav, "empty.sqlite3"),
        (&corrupt_wav, "corrupt.sqlite3"),
    ] {
        let db_path = temp.path().join(db_name);
        let error = run_transcription_job(&engine, input.to_path_buf(), db_path, &config)
            .expect_err("invalid wav must fail");
        assert!(
            matches!(error, AppError::Transcription(_)),
            "unexpected error type: {error}"
        );
    }
}

fn write_wrapper(dir: &Path, name: &str, target: &Path) {
    let script_path = dir.join(name);
    let script = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", target.display());
    fs::write(&script_path, script).unwrap_or_else(|error| {
        panic!("failed to write wrapper {}: {error}", script_path.display())
    });

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script_path)
            .unwrap_or_else(|error| panic!("metadata {}: {error}", script_path.display()))
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions)
            .unwrap_or_else(|error| panic!("chmod {}: {error}", script_path.display()));
    }
}

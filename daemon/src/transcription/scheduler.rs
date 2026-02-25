use std::path::PathBuf;

use franken_whisper::BackendKind;
use serde::Serialize;

use crate::config::TranscriptionConfig;
use crate::error::AppResult;
use crate::transcription::engine::EngineAdapter;
use crate::transcription::request_builder::build_request;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptResult {
    pub run_id: String,
    pub backend: BackendKind,
    pub transcript: String,
    pub language: Option<String>,
    pub warnings: Vec<String>,
    pub finished_at_rfc3339: String,
}

pub fn run_transcription_job(
    engine: &impl EngineAdapter,
    wav_path: PathBuf,
    db_path: PathBuf,
    config: &TranscriptionConfig,
) -> AppResult<TranscriptResult> {
    let request = build_request(wav_path, db_path, config);
    let report = engine.transcribe_request(request)?;

    Ok(TranscriptResult {
        run_id: report.run_id,
        backend: report.result.backend,
        transcript: report.result.transcript,
        language: report.result.language,
        warnings: report.warnings,
        finished_at_rfc3339: report.finished_at_rfc3339,
    })
}

#[cfg(test)]
mod tests {
    use super::run_transcription_job;
    use crate::config::schema::TranscriptionConfig;
    use crate::error::{AppError, AppResult};
    use crate::transcription::engine::EngineAdapter;
    use franken_whisper::model::{
        BackendKind, InputSource, ReplayEnvelope, RunEvent, RunReport, TranscribeRequest,
        TranscriptionResult,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeEngine {
        requests: Mutex<Vec<TranscribeRequest>>,
        result: Mutex<Option<AppResult<RunReport>>>,
    }

    impl FakeEngine {
        fn with_result(result: AppResult<RunReport>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                result: Mutex::new(Some(result)),
            }
        }
    }

    impl EngineAdapter for FakeEngine {
        fn transcribe_request(&self, request: TranscribeRequest) -> AppResult<RunReport> {
            self.requests.lock().expect("lock").push(request);
            self.result
                .lock()
                .expect("lock")
                .take()
                .expect("configured result")
        }
    }

    fn sample_report() -> RunReport {
        RunReport {
            run_id: "run-123".to_owned(),
            trace_id: "trace-1".to_owned(),
            started_at_rfc3339: "2026-02-25T00:00:00Z".to_owned(),
            finished_at_rfc3339: "2026-02-25T00:00:02Z".to_owned(),
            input_path: "/tmp/in.wav".to_owned(),
            normalized_wav_path: "/tmp/normalized.wav".to_owned(),
            request: TranscribeRequest {
                input: InputSource::File {
                    path: PathBuf::from("/tmp/in.wav"),
                },
                backend: BackendKind::WhisperCpp,
                model: Some("base".to_owned()),
                language: Some("en".to_owned()),
                translate: false,
                diarize: false,
                persist: true,
                db_path: PathBuf::from("/tmp/history.sqlite3"),
                timeout_ms: Some(1_000),
                backend_params: Default::default(),
            },
            result: TranscriptionResult {
                backend: BackendKind::WhisperCpp,
                transcript: "hello world".to_owned(),
                language: Some("en".to_owned()),
                segments: vec![],
                acceleration: None,
                raw_output: json!({}),
                artifact_paths: vec![],
            },
            events: vec![RunEvent {
                seq: 1,
                ts_rfc3339: "2026-02-25T00:00:01Z".to_owned(),
                stage: "backend".to_owned(),
                code: "done".to_owned(),
                message: "ok".to_owned(),
                payload: json!({}),
            }],
            warnings: vec!["minor".to_owned()],
            evidence: vec![],
            replay: ReplayEnvelope::default(),
        }
    }

    #[test]
    fn maps_run_report_to_transcript_result() {
        let engine = FakeEngine::with_result(Ok(sample_report()));
        let config = TranscriptionConfig::default();
        let output = run_transcription_job(
            &engine,
            PathBuf::from("/tmp/in.wav"),
            PathBuf::from("/tmp/history.sqlite3"),
            &config,
        )
        .expect("success");

        assert_eq!(output.run_id, "run-123");
        assert_eq!(output.backend, BackendKind::WhisperCpp);
        assert_eq!(output.transcript, "hello world");
        assert_eq!(output.language.as_deref(), Some("en"));
        assert_eq!(output.warnings, vec!["minor".to_owned()]);
        assert_eq!(output.finished_at_rfc3339, "2026-02-25T00:00:02Z");
    }

    #[test]
    fn propagates_engine_failures() {
        let engine = FakeEngine::with_result(Err(AppError::Transcription("timeout".to_owned())));
        let config = TranscriptionConfig::default();
        let error = run_transcription_job(
            &engine,
            PathBuf::from("/tmp/in.wav"),
            PathBuf::from("/tmp/history.sqlite3"),
            &config,
        )
        .expect_err("must fail");
        assert!(matches!(error, AppError::Transcription(message) if message.contains("timeout")));
    }
}

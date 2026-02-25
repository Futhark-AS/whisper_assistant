use std::path::PathBuf;

use franken_whisper::BackendKind;
use serde::Serialize;

use crate::config::TranscriptionConfig;
use crate::error::AppResult;
use crate::transcription::engine::FrankenEngine;
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
    engine: &FrankenEngine,
    wav_path: PathBuf,
    db_path: PathBuf,
    config: &TranscriptionConfig,
) -> AppResult<TranscriptResult> {
    let request = build_request(wav_path, db_path, config);
    let report = engine.transcribe(request)?;

    Ok(TranscriptResult {
        run_id: report.run_id,
        backend: report.result.backend,
        transcript: report.result.transcript,
        language: report.result.language,
        warnings: report.warnings,
        finished_at_rfc3339: report.finished_at_rfc3339,
    })
}

use std::path::PathBuf;

use franken_whisper::model::{BackendParams, InputSource};
use franken_whisper::TranscribeRequest;

use crate::config::TranscriptionConfig;

pub fn build_request(
    wav_path: PathBuf,
    db_path: PathBuf,
    cfg: &TranscriptionConfig,
) -> TranscribeRequest {
    TranscribeRequest {
        input: InputSource::File { path: wav_path },
        backend: cfg.backend,
        model: cfg.model_id.clone(),
        language: cfg.language.clone(),
        translate: cfg.translate,
        diarize: cfg.diarize,
        persist: true,
        db_path,
        timeout_ms: Some(cfg.timeout_ms()),
        backend_params: BackendParams {
            threads: cfg.threads,
            processors: cfg.processors,
            ..BackendParams::default()
        },
    }
}

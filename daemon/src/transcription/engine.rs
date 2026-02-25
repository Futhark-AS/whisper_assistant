use franken_whisper::{FrankenWhisperEngine, RunReport, TranscribeRequest};

use crate::error::{AppError, AppResult};

pub trait EngineAdapter {
    fn transcribe_request(&self, request: TranscribeRequest) -> AppResult<RunReport>;
}

pub struct FrankenEngine {
    inner: FrankenWhisperEngine,
}

impl FrankenEngine {
    pub fn new() -> AppResult<Self> {
        let inner = FrankenWhisperEngine::new()
            .map_err(|error| AppError::Transcription(format!("engine init failed: {error}")))?;
        Ok(Self { inner })
    }

    pub fn transcribe(&self, request: TranscribeRequest) -> AppResult<RunReport> {
        self.inner
            .transcribe(request)
            .map_err(|error| AppError::Transcription(format!("engine transcribe failed: {error}")))
    }
}

impl EngineAdapter for FrankenEngine {
    fn transcribe_request(&self, request: TranscribeRequest) -> AppResult<RunReport> {
        self.transcribe(request)
    }
}

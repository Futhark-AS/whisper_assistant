use franken_whisper::{FrankenWhisperEngine, RunReport, TranscribeRequest};
use std::fmt::Display;

use crate::error::{AppError, AppResult};

pub trait EngineAdapter {
    fn transcribe_request(&self, request: TranscribeRequest) -> AppResult<RunReport>;
}

pub struct FrankenEngine {
    inner: FrankenWhisperEngine,
}

impl FrankenEngine {
    pub fn new() -> AppResult<Self> {
        Self::try_new_with(FrankenWhisperEngine::new)
    }

    fn try_new_with<F, E>(init_engine: F) -> AppResult<Self>
    where
        F: FnOnce() -> Result<FrankenWhisperEngine, E>,
        E: Display,
    {
        let inner = init_engine().map_err(|error| map_init_error(&error))?;
        Ok(Self { inner })
    }

    pub fn transcribe(&self, request: TranscribeRequest) -> AppResult<RunReport> {
        map_transcribe_result(self.inner.transcribe(request))
    }
}

impl EngineAdapter for FrankenEngine {
    fn transcribe_request(&self, request: TranscribeRequest) -> AppResult<RunReport> {
        self.transcribe(request)
    }
}

fn map_init_error(error: &impl Display) -> AppError {
    AppError::Transcription(format!("engine init failed: {error}"))
}

fn map_transcribe_result<T, E>(result: Result<T, E>) -> AppResult<T>
where
    E: Display,
{
    result.map_err(|error| AppError::Transcription(format!("engine transcribe failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{map_transcribe_result, FrankenEngine};
    use crate::error::AppError;

    #[test]
    fn init_error_mapping_uses_stable_prefix() {
        let error = match FrankenEngine::try_new_with(
            || -> Result<franken_whisper::FrankenWhisperEngine, &str> { Err("boom") },
        ) {
            Ok(_) => panic!("init should fail"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            AppError::Transcription(message) if message == "engine init failed: boom"
        ));
    }

    #[test]
    fn transcribe_error_mapping_uses_stable_prefix() {
        let error = map_transcribe_result::<(), _>(Err("timeout")).expect_err("must fail");
        assert!(matches!(
            error,
            AppError::Transcription(message) if message == "engine transcribe failed: timeout"
        ));
    }
}

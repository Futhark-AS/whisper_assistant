use std::path::PathBuf;

use serde::Serialize;

use crate::controller::state::ControllerState;
use crate::doctor::DoctorReport;
use crate::transcription::TranscriptResult;

#[derive(Debug, Clone)]
pub enum ControllerEvent {
    Toggle,
    RunDoctor,
    Tick,
    Shutdown,
    TranscriptionFinished {
        wav_path: PathBuf,
        result: Result<TranscriptResult, String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControllerOutput {
    StateChanged(ControllerState),
    Notification(String),
    DoctorReport(DoctorReport),
    TranscriptReady(TranscriptResult),
    Stopped,
}

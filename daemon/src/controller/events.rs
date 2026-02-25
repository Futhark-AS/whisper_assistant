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
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ControllerOutput {
    StateChanged(ControllerState),
    Notification(String),
    DoctorReport(DoctorReport),
    TranscriptReady(TranscriptResult),
    Stopped,
}

#[cfg(test)]
mod tests {
    use super::ControllerOutput;
    use crate::controller::state::ControllerState;
    use crate::doctor::report::{CheckResult, CheckStatus, DoctorReport, DoctorState};
    use crate::transcription::scheduler::TranscriptResult;
    use franken_whisper::BackendKind;

    #[test]
    fn controller_output_json_shape_round_trip() {
        let report = DoctorReport {
            generated_at_rfc3339: "2026-02-25T00:00:00Z".to_owned(),
            state: DoctorState::Ready,
            checks: vec![CheckResult {
                name: "ffmpeg".to_owned(),
                status: CheckStatus::Pass,
                detail: "ok".to_owned(),
                required: true,
                remediation: None,
            }],
        };
        let transcript = TranscriptResult {
            run_id: "run-1".to_owned(),
            backend: BackendKind::WhisperCpp,
            transcript: "hello".to_owned(),
            language: Some("en".to_owned()),
            warnings: vec!["warn".to_owned()],
            finished_at_rfc3339: "2026-02-25T00:00:01Z".to_owned(),
        };

        for output in [
            ControllerOutput::StateChanged(ControllerState::Idle),
            ControllerOutput::Notification("note".to_owned()),
            ControllerOutput::DoctorReport(report.clone()),
            ControllerOutput::TranscriptReady(transcript.clone()),
            ControllerOutput::Stopped,
        ] {
            let json = serde_json::to_string(&output).expect("serialize");
            let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
            assert!(parsed.get("type").is_some());
        }
    }
}

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
    use serde_json::Value;

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

        let state_changed = serde_json::to_value(ControllerOutput::StateChanged(
            ControllerState::Degraded("missing ffmpeg".to_owned()),
        ))
        .expect("serialize");
        assert_eq!(
            state_changed.get("type").and_then(Value::as_str),
            Some("state_changed")
        );
        assert_eq!(
            state_changed
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("mode"))
                .and_then(Value::as_str),
            Some("degraded")
        );
        assert_eq!(
            state_changed
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("reason"))
                .and_then(Value::as_str),
            Some("missing ffmpeg")
        );

        let notification = serde_json::to_value(ControllerOutput::Notification("note".to_owned()))
            .expect("serialize");
        assert_eq!(
            notification.get("type").and_then(Value::as_str),
            Some("notification")
        );
        assert_eq!(
            notification.get("payload").and_then(Value::as_str),
            Some("note")
        );

        let doctor = serde_json::to_value(ControllerOutput::DoctorReport(report.clone()))
            .expect("serialize");
        assert_eq!(
            doctor.get("type").and_then(Value::as_str),
            Some("doctor_report")
        );
        assert_eq!(
            doctor
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("state"))
                .and_then(Value::as_str),
            Some("ready")
        );
        let checks = doctor
            .get("payload")
            .and_then(Value::as_object)
            .and_then(|payload| payload.get("checks"))
            .and_then(Value::as_array)
            .expect("doctor checks");
        assert_eq!(checks.len(), 1);
        assert_eq!(
            checks[0].get("name").and_then(Value::as_str),
            Some("ffmpeg")
        );
        assert_eq!(
            checks[0].get("status").and_then(Value::as_str),
            Some("pass")
        );
        assert_eq!(
            checks[0].get("required").and_then(Value::as_bool),
            Some(true)
        );

        let transcript_ready =
            serde_json::to_value(ControllerOutput::TranscriptReady(transcript.clone()))
                .expect("serialize");
        assert_eq!(
            transcript_ready.get("type").and_then(Value::as_str),
            Some("transcript_ready")
        );
        let transcript_payload = transcript_ready
            .get("payload")
            .and_then(Value::as_object)
            .expect("transcript payload");
        assert_eq!(
            transcript_payload.get("run_id").and_then(Value::as_str),
            Some("run-1")
        );
        assert_eq!(
            transcript_payload.get("backend").and_then(Value::as_str),
            Some("whisper_cpp")
        );
        assert_eq!(
            transcript_payload.get("transcript").and_then(Value::as_str),
            Some("hello")
        );
        assert_eq!(
            transcript_payload.get("language").and_then(Value::as_str),
            Some("en")
        );
        assert_eq!(
            transcript_payload
                .get("warnings")
                .and_then(Value::as_array)
                .and_then(|warnings| warnings.first())
                .and_then(Value::as_str),
            Some("warn")
        );

        let stopped =
            serde_json::to_value(ControllerOutput::Stopped).expect("serialize stopped output");
        assert_eq!(stopped.get("type").and_then(Value::as_str), Some("stopped"));
        assert!(stopped.get("payload").is_none());
    }
}

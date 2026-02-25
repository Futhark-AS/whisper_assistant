use franken_whisper::BackendKind;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub started_at_rfc3339: String,
    pub finished_at_rfc3339: String,
    pub backend: BackendKind,
    pub transcript_preview: String,
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "mode", content = "reason", rename_all = "snake_case")]
pub enum ControllerState {
    Idle,
    Recording,
    Processing,
    Degraded(String),
}

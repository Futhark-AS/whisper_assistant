use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", content = "reason", rename_all = "snake_case")]
pub enum ControllerState {
    Idle,
    Recording,
    Processing,
    Degraded(String),
}

#[cfg(test)]
mod tests {
    use super::ControllerState;

    #[test]
    fn controller_state_json_shape_round_trip() {
        let cases = vec![
            ControllerState::Idle,
            ControllerState::Recording,
            ControllerState::Processing,
            ControllerState::Degraded("oops".to_owned()),
        ];

        for state in cases {
            let json = serde_json::to_string(&state).expect("serialize");
            let value: serde_json::Value = serde_json::from_str(&json).expect("json");
            assert!(value.get("mode").is_some());
            let parsed: ControllerState = serde_json::from_str(&json).expect("deserialize");
            match (state, parsed) {
                (ControllerState::Degraded(a), ControllerState::Degraded(b)) => assert_eq!(a, b),
                (lhs, rhs) => assert_eq!(format!("{lhs:?}"), format!("{rhs:?}")),
            }
        }
    }
}

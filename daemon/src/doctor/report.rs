use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoctorState {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    pub required: bool,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub generated_at_rfc3339: String,
    pub state: DoctorState,
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Doctor state: {:?}\n", self.state));
        out.push_str(&format!("Generated at: {}\n\n", self.generated_at_rfc3339));
        out.push_str(&format!("{:<30} {:<8} {:<8} {}\n", "CHECK", "STATUS", "REQUIRED", "DETAIL"));
        out.push_str(&format!("{:<30} {:<8} {:<8} {}\n", "-----", "------", "--------", "------"));

        for check in &self.checks {
            out.push_str(&format!(
                "{:<30} {:<8} {:<8} {}\n",
                check.name,
                status_label(check.status),
                if check.required { "yes" } else { "no" },
                check.detail
            ));
            if let Some(remediation) = &check.remediation {
                out.push_str(&format!("  remediation: {}\n", remediation));
            }
        }

        out
    }
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "PASS",
        CheckStatus::Warn => "WARN",
        CheckStatus::Fail => "FAIL",
        CheckStatus::Skip => "SKIP",
    }
}

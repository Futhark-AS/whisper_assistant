use notify_rust::Notification;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct Notifier {
    enabled: bool,
}

impl Notifier {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn notify(&self, summary: &str, body: &str) -> AppResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let _ = Notification::new().summary(summary).body(body).show();
        Ok(())
    }
}

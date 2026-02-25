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

#[cfg(test)]
mod tests {
    use super::Notifier;

    #[test]
    fn disabled_notifier_is_noop_success() {
        let notifier = Notifier::new(false);
        notifier.notify("Title", "Body").expect("disabled notify");
    }

    #[test]
    fn enabled_notifier_does_not_propagate_backend_errors() {
        let notifier = Notifier::new(true);
        notifier.notify("Title", "Body").expect("enabled notify");
    }
}

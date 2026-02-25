use notify_rust::Notification;
use std::sync::Arc;

use crate::error::AppResult;

pub struct Notifier {
    enabled: bool,
    backend: Arc<dyn NotificationBackend>,
}

impl std::fmt::Debug for Notifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifier")
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Clone for Notifier {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            backend: self.backend.clone(),
        }
    }
}

trait NotificationBackend: Send + Sync {
    fn show(&self, summary: &str, body: &str) -> Result<(), String>;
}

struct NotifyRustBackend;

impl NotificationBackend for NotifyRustBackend {
    fn show(&self, summary: &str, body: &str) -> Result<(), String> {
        Notification::new()
            .summary(summary)
            .body(body)
            .show()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl Notifier {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            backend: Arc::new(NotifyRustBackend),
        }
    }

    #[cfg(test)]
    fn with_backend(enabled: bool, backend: Arc<dyn NotificationBackend>) -> Self {
        Self { enabled, backend }
    }

    pub fn notify(&self, summary: &str, body: &str) -> AppResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let _ = self.backend.show(summary, body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Notifier;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeNotificationBackend {
        calls: Mutex<Vec<(String, String)>>,
        fail: bool,
    }

    impl super::NotificationBackend for FakeNotificationBackend {
        fn show(&self, summary: &str, body: &str) -> Result<(), String> {
            self.calls
                .lock()
                .expect("lock calls")
                .push((summary.to_owned(), body.to_owned()));
            if self.fail {
                return Err("backend unavailable".to_owned());
            }
            Ok(())
        }
    }

    #[test]
    fn disabled_notifier_is_noop_success() {
        let backend = Arc::new(FakeNotificationBackend::default());
        let notifier = Notifier::with_backend(false, backend.clone());
        notifier.notify("Title", "Body").expect("disabled notify");
        assert!(
            backend.calls.lock().expect("lock calls").is_empty(),
            "disabled notifier should not call backend"
        );
    }

    #[test]
    fn enabled_notifier_does_not_propagate_backend_errors() {
        let backend = Arc::new(FakeNotificationBackend {
            calls: Mutex::new(Vec::new()),
            fail: true,
        });
        let notifier = Notifier::with_backend(true, backend.clone());
        notifier.notify("Title", "Body").expect("enabled notify");
        assert_eq!(
            backend.calls.lock().expect("lock calls").as_slice(),
            [("Title".to_owned(), "Body".to_owned())]
        );
    }
}

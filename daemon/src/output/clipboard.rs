use arboard::Clipboard;

use crate::error::{AppError, AppResult};

pub struct ClipboardOutput;

trait ClipboardBackend {
    fn set_text(&mut self, text: String) -> Result<(), String>;
}

struct ArboardClipboardBackend {
    inner: Clipboard,
}

impl ClipboardBackend for ArboardClipboardBackend {
    fn set_text(&mut self, text: String) -> Result<(), String> {
        self.inner.set_text(text).map_err(|error| error.to_string())
    }
}

impl ClipboardOutput {
    pub fn write_text(text: &str) -> AppResult<()> {
        Self::write_text_with(text, || {
            let inner = Clipboard::new()
                .map_err(|error| AppError::Clipboard(format!("clipboard init failed: {error}")))?;
            Ok(Box::new(ArboardClipboardBackend { inner }) as Box<dyn ClipboardBackend>)
        })
    }

    fn write_text_with<F>(text: &str, mut make_backend: F) -> AppResult<()>
    where
        F: FnMut() -> AppResult<Box<dyn ClipboardBackend>>,
    {
        let mut backend = make_backend()?;
        backend
            .set_text(text.to_owned())
            .map_err(|error| AppError::Clipboard(format!("clipboard write failed: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardOutput;
    use crate::error::AppError;
    use std::sync::{Arc, Mutex};

    struct FakeClipboardBackend {
        writes: Arc<Mutex<Vec<String>>>,
        fail_with: Option<String>,
    }

    impl super::ClipboardBackend for FakeClipboardBackend {
        fn set_text(&mut self, text: String) -> Result<(), String> {
            self.writes.lock().expect("lock writes").push(text);
            if let Some(error) = self.fail_with.take() {
                return Err(error);
            }
            Ok(())
        }
    }

    #[test]
    fn write_text_reports_init_failure_with_stable_prefix() {
        let error = ClipboardOutput::write_text_with("hello world", || {
            Err(AppError::Clipboard(
                "clipboard init failed: no display".to_owned(),
            ))
        })
        .expect_err("init must fail");
        assert!(matches!(
            error,
            AppError::Clipboard(message) if message.starts_with("clipboard init failed: ")
        ));
    }

    #[test]
    fn write_text_reports_write_failure_with_stable_prefix() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let writes_for_backend = writes.clone();
        let error = ClipboardOutput::write_text_with("hello world", move || {
            Ok(Box::new(FakeClipboardBackend {
                writes: writes_for_backend.clone(),
                fail_with: Some("permission denied".to_owned()),
            }) as Box<dyn super::ClipboardBackend>)
        })
        .expect_err("write must fail");

        assert!(matches!(
            error,
            AppError::Clipboard(message)
                if message == "clipboard write failed: permission denied"
        ));
        assert_eq!(
            writes.lock().expect("lock writes").as_slice(),
            ["hello world"]
        );
    }

    #[test]
    fn write_text_succeeds_with_fake_backend() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let writes_for_backend = writes.clone();
        ClipboardOutput::write_text_with("hello world", move || {
            Ok(Box::new(FakeClipboardBackend {
                writes: writes_for_backend.clone(),
                fail_with: None,
            }) as Box<dyn super::ClipboardBackend>)
        })
        .expect("write should succeed");

        assert_eq!(
            writes.lock().expect("lock writes").as_slice(),
            ["hello world"]
        );
    }
}

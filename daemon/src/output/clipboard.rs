use arboard::Clipboard;

use crate::error::{AppError, AppResult};

pub struct ClipboardOutput;

impl ClipboardOutput {
    pub fn write_text(text: &str) -> AppResult<()> {
        let mut clipboard = Clipboard::new()
            .map_err(|error| AppError::Clipboard(format!("clipboard init failed: {error}")))?;
        clipboard
            .set_text(text.to_owned())
            .map_err(|error| AppError::Clipboard(format!("clipboard write failed: {error}")))
    }
}

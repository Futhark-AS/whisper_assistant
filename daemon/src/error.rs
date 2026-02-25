use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("toml serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("binary `{binary}` missing from PATH")]
    BinaryMissing { binary: String },

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("capture failed: {0}")]
    Capture(String),

    #[error("transcription failed: {0}")]
    Transcription(String),

    #[error("clipboard output failed: {0}")]
    Clipboard(String),

    #[error("controller error: {0}")]
    Controller(String),

    #[error("channel closed: {0}")]
    ChannelClosed(String),

    #[error("install failed: {0}")]
    Install(String),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::AppError;
    use serde::ser::Error as _;

    #[test]
    fn display_messages_cover_all_variants() {
        let cases = vec![
            AppError::Io(std::io::Error::other("disk gone")),
            AppError::TomlParse(toml::from_str::<toml::Value>("not= [valid").unwrap_err()),
            AppError::TomlSerialize(toml::ser::Error::custom("serialize failed")),
            AppError::Json(serde_json::from_str::<serde_json::Value>("{bad").unwrap_err()),
            AppError::BinaryMissing {
                binary: "ffmpeg".to_owned(),
            },
            AppError::Config("bad config".to_owned()),
            AppError::Capture("capture boom".to_owned()),
            AppError::Transcription("tx failed".to_owned()),
            AppError::Clipboard("clipboard dead".to_owned()),
            AppError::Controller("controller dead".to_owned()),
            AppError::ChannelClosed("closed".to_owned()),
            AppError::Install("install failed".to_owned()),
            AppError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ErrorCode::Unknown,
                    extended_code: 1,
                },
                Some("sqlite boom".to_owned()),
            )),
        ];

        for error in cases {
            let display = format!("{error}");
            let debug = format!("{error:?}");
            assert!(!display.trim().is_empty());
            assert!(!debug.trim().is_empty());
        }
    }
}

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

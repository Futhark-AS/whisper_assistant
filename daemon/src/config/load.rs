use std::path::PathBuf;

use franken_whisper::BackendKind;

use crate::bootstrap::AppPaths;
use crate::config::schema::{AppConfig, OutputMode};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub config_path: Option<PathBuf>,
    pub backend: Option<BackendKind>,
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub diarize: Option<bool>,
    pub translate: Option<bool>,
    pub hotkey_binding: Option<String>,
    pub output_mode: Option<OutputMode>,
}

pub fn load_config(paths: &AppPaths, overrides: &CliOverrides) -> AppResult<AppConfig> {
    let config_path = overrides
        .config_path
        .clone()
        .unwrap_or_else(|| paths.config_file.clone());

    let mut config = if config_path.exists() {
        let raw = std::fs::read_to_string(&config_path)?;
        toml::from_str::<AppConfig>(&raw)?
    } else {
        let defaults = AppConfig::default();
        write_default_config(&config_path, &defaults)?;
        defaults
    };

    if config.history.db_path.is_none() {
        config.history.db_path = Some(paths.history_db.clone());
    }

    apply_env_overrides(&mut config);
    apply_cli_overrides(&mut config, overrides);

    validate(&config)?;
    Ok(config)
}

fn write_default_config(path: &PathBuf, defaults: &AppConfig) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let data = toml::to_string_pretty(defaults)?;
    std::fs::write(path, data)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }

    Ok(())
}

fn validate(config: &AppConfig) -> AppResult<()> {
    if config.transcription.timeout_seconds == 0 {
        return Err(AppError::Config(
            "transcription.timeout_seconds must be > 0".to_owned(),
        ));
    }

    if config.audio.max_recording_seconds == 0 {
        return Err(AppError::Config(
            "audio.max_recording_seconds must be > 0".to_owned(),
        ));
    }

    Ok(())
}

fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(value) = std::env::var("QUEDO_BACKEND") {
        if let Some(parsed) = parse_backend_kind(&value) {
            config.transcription.backend = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_MODEL_ID") {
        config.transcription.model_id = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
    }
    if let Ok(value) = std::env::var("QUEDO_LANGUAGE") {
        config.transcription.language = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
    }
    if let Ok(value) = std::env::var("QUEDO_TRANSLATE") {
        if let Some(parsed) = parse_bool(&value) {
            config.transcription.translate = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_DIARIZE") {
        if let Some(parsed) = parse_bool(&value) {
            config.transcription.diarize = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_TIMEOUT_SECONDS") {
        if let Ok(parsed) = value.parse::<u64>() {
            config.transcription.timeout_seconds = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_OUTPUT_MODE") {
        if let Some(parsed) = parse_output_mode(&value) {
            config.output.mode = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_HOTKEY_BINDING") {
        config.hotkey.binding = value;
    }
    if let Ok(value) = std::env::var("QUEDO_HISTORY_DB_PATH") {
        if !value.trim().is_empty() {
            config.history.db_path = Some(PathBuf::from(value));
        }
    }
    if let Ok(value) = std::env::var("QUEDO_AUTOSTART_ENABLED") {
        if let Some(parsed) = parse_bool(&value) {
            config.service.autostart_enabled = parsed;
        }
    }
    if let Ok(value) = std::env::var("QUEDO_LOG_LEVEL") {
        config.diagnostics.log_level = value;
    }
    if let Ok(value) = std::env::var("QUEDO_MAX_RECORDING_SECONDS") {
        if let Ok(parsed) = value.parse::<u32>() {
            config.audio.max_recording_seconds = parsed;
        }
    }
}

fn apply_cli_overrides(config: &mut AppConfig, overrides: &CliOverrides) {
    if let Some(value) = overrides.backend {
        config.transcription.backend = value;
    }
    if let Some(value) = &overrides.model_id {
        config.transcription.model_id = Some(value.clone());
    }
    if let Some(value) = &overrides.language {
        config.transcription.language = Some(value.clone());
    }
    if let Some(value) = overrides.timeout_seconds {
        config.transcription.timeout_seconds = value;
    }
    if let Some(value) = overrides.diarize {
        config.transcription.diarize = value;
    }
    if let Some(value) = overrides.translate {
        config.transcription.translate = value;
    }
    if let Some(value) = &overrides.hotkey_binding {
        config.hotkey.binding = value.clone();
    }
    if let Some(value) = &overrides.output_mode {
        config.output.mode = value.clone();
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_backend_kind(value: &str) -> Option<BackendKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(BackendKind::Auto),
        "whisper_cpp" | "whisper-cpp" => Some(BackendKind::WhisperCpp),
        "insanely_fast" | "insanely-fast" => Some(BackendKind::InsanelyFast),
        "whisper_diarization" | "whisper-diarization" => Some(BackendKind::WhisperDiarization),
        _ => None,
    }
}

fn parse_output_mode(value: &str) -> Option<OutputMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "clipboard_only" | "clipboard-only" => Some(OutputMode::ClipboardOnly),
        "disabled" | "none" => Some(OutputMode::Disabled),
        _ => None,
    }
}

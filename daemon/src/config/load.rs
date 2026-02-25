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

#[cfg(test)]
mod tests {
    use super::{
        apply_cli_overrides, apply_env_overrides, load_config, parse_backend_kind, parse_bool,
        parse_output_mode, validate, CliOverrides,
    };
    use crate::bootstrap::paths::AppPaths;
    use crate::config::schema::{AppConfig, OutputMode};
    use crate::error::AppError;
    use franken_whisper::BackendKind;
    use std::path::{Path, PathBuf};

    struct EnvVarGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old }
        }

        fn clear(key: &'static str) -> Self {
            let old = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.old.as_ref() {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn paths_for(root: &Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            logs_dir: root.join("cache/logs"),
            state_dir: root.join("cache/fw-state"),
            config_file: root.join("config/config.toml"),
            history_db: root.join("data/history.sqlite3"),
            autostart_file: root.join("autostart/quedo-daemon.desktop"),
        }
    }

    fn clear_quedo_env() -> Vec<EnvVarGuard> {
        [
            "QUEDO_BACKEND",
            "QUEDO_MODEL_ID",
            "QUEDO_LANGUAGE",
            "QUEDO_TRANSLATE",
            "QUEDO_DIARIZE",
            "QUEDO_TIMEOUT_SECONDS",
            "QUEDO_OUTPUT_MODE",
            "QUEDO_HOTKEY_BINDING",
            "QUEDO_HISTORY_DB_PATH",
            "QUEDO_AUTOSTART_ENABLED",
            "QUEDO_LOG_LEVEL",
            "QUEDO_MAX_RECORDING_SECONDS",
        ]
        .iter()
        .map(|key| EnvVarGuard::clear(key))
        .collect()
    }

    #[test]
    fn missing_config_file_writes_defaults() {
        let _guard = crate::test_support::lock_env();
        let _clean = clear_quedo_env();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let paths = paths_for(tmp.path());
        paths.ensure_dirs().expect("dirs");
        assert!(!paths.config_file.exists());

        let config = load_config(&paths, &CliOverrides::default()).expect("load config");
        assert!(paths.config_file.exists());
        assert_eq!(config.history.db_path, Some(paths.history_db.clone()));
    }

    #[test]
    fn precedence_toml_then_env_then_cli() {
        let _guard = crate::test_support::lock_env();
        let _clean = clear_quedo_env();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let paths = paths_for(tmp.path());
        paths.ensure_dirs().expect("dirs");
        let config_toml = r#"
[transcription]
backend = "auto"
model_id = "from_toml"
timeout_seconds = 11
diarize = false
translate = false
language = "de"

[output]
mode = "disabled"
"#;
        std::fs::write(&paths.config_file, config_toml).expect("write config");

        let _backend = EnvVarGuard::set("QUEDO_BACKEND", "insanely_fast");
        let _model = EnvVarGuard::set("QUEDO_MODEL_ID", "from_env");
        let _timeout = EnvVarGuard::set("QUEDO_TIMEOUT_SECONDS", "22");
        let _diarize = EnvVarGuard::set("QUEDO_DIARIZE", "true");

        let overrides = CliOverrides {
            backend: Some(BackendKind::WhisperCpp),
            model_id: Some("from_cli".to_owned()),
            timeout_seconds: Some(33),
            diarize: Some(false),
            output_mode: Some(OutputMode::ClipboardOnly),
            ..CliOverrides::default()
        };

        let config = load_config(&paths, &overrides).expect("load config");
        assert_eq!(config.transcription.backend, BackendKind::WhisperCpp);
        assert_eq!(config.transcription.model_id.as_deref(), Some("from_cli"));
        assert_eq!(config.transcription.timeout_seconds, 33);
        assert!(!config.transcription.diarize);
        assert_eq!(config.output.mode, OutputMode::ClipboardOnly);
    }

    #[test]
    fn validate_rejects_zero_timeout_and_max_recording() {
        let mut config = AppConfig::default();
        config.transcription.timeout_seconds = 0;
        assert!(matches!(validate(&config), Err(AppError::Config(message)) if message.contains("timeout_seconds")));

        config.transcription.timeout_seconds = 1;
        config.audio.max_recording_seconds = 0;
        assert!(
            matches!(validate(&config), Err(AppError::Config(message)) if message.contains("max_recording_seconds"))
        );
    }

    #[test]
    fn missing_optional_fields_are_filled_from_defaults() {
        let _guard = crate::test_support::lock_env();
        let _clean = clear_quedo_env();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let paths = paths_for(tmp.path());
        paths.ensure_dirs().expect("dirs");
        std::fs::write(
            &paths.config_file,
            r#"[transcription]
timeout_seconds = 99
"#,
        )
        .expect("write");

        let config = load_config(&paths, &CliOverrides::default()).expect("load");
        assert_eq!(config.transcription.timeout_seconds, 99);
        assert_eq!(config.output.mode, OutputMode::ClipboardOnly);
        assert_eq!(config.hotkey.binding, "Ctrl+Shift+Space");
    }

    #[test]
    fn parse_type_mismatch_fails() {
        let _guard = crate::test_support::lock_env();
        let _clean = clear_quedo_env();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let paths = paths_for(tmp.path());
        paths.ensure_dirs().expect("dirs");
        std::fs::write(
            &paths.config_file,
            r#"[transcription]
timeout_seconds = "abc"
"#,
        )
        .expect("write");

        let error = load_config(&paths, &CliOverrides::default()).expect_err("must fail");
        assert!(matches!(error, AppError::TomlParse(_)));
    }

    #[test]
    fn parse_bool_supports_canonical_values() {
        let truthy = ["1", "true", "yes", "on", " TRUE "];
        let falsy = ["0", "false", "no", "off", " Off "];
        for value in truthy {
            assert_eq!(parse_bool(value), Some(true), "{value}");
        }
        for value in falsy {
            assert_eq!(parse_bool(value), Some(false), "{value}");
        }
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn backend_parser_supports_labels_and_aliases() {
        assert_eq!(parse_backend_kind("auto"), Some(BackendKind::Auto));
        assert_eq!(
            parse_backend_kind("whisper_cpp"),
            Some(BackendKind::WhisperCpp)
        );
        assert_eq!(
            parse_backend_kind("whisper-cpp"),
            Some(BackendKind::WhisperCpp)
        );
        assert_eq!(
            parse_backend_kind("insanely_fast"),
            Some(BackendKind::InsanelyFast)
        );
        assert_eq!(
            parse_backend_kind("insanely-fast"),
            Some(BackendKind::InsanelyFast)
        );
        assert_eq!(
            parse_backend_kind("whisper_diarization"),
            Some(BackendKind::WhisperDiarization)
        );
        assert_eq!(
            parse_backend_kind("whisper-diarization"),
            Some(BackendKind::WhisperDiarization)
        );
        assert_eq!(parse_backend_kind("nope"), None);
    }

    #[test]
    fn output_mode_parser_supports_aliases() {
        assert_eq!(parse_output_mode("clipboard_only"), Some(OutputMode::ClipboardOnly));
        assert_eq!(parse_output_mode("clipboard-only"), Some(OutputMode::ClipboardOnly));
        assert_eq!(parse_output_mode("disabled"), Some(OutputMode::Disabled));
        assert_eq!(parse_output_mode("none"), Some(OutputMode::Disabled));
        assert_eq!(parse_output_mode("other"), None);
    }

    #[test]
    fn env_overrides_update_fields() {
        let _guard = crate::test_support::lock_env();
        let _clean = clear_quedo_env();
        let _backend = EnvVarGuard::set("QUEDO_BACKEND", "whisper_cpp");
        let _model = EnvVarGuard::set("QUEDO_MODEL_ID", "m1");
        let _language = EnvVarGuard::set("QUEDO_LANGUAGE", "en");
        let _translate = EnvVarGuard::set("QUEDO_TRANSLATE", "yes");
        let _diarize = EnvVarGuard::set("QUEDO_DIARIZE", "true");
        let _timeout = EnvVarGuard::set("QUEDO_TIMEOUT_SECONDS", "77");
        let _output = EnvVarGuard::set("QUEDO_OUTPUT_MODE", "disabled");
        let _hotkey = EnvVarGuard::set("QUEDO_HOTKEY_BINDING", "Ctrl+Alt+Q");
        let _history = EnvVarGuard::set("QUEDO_HISTORY_DB_PATH", "/tmp/h.sqlite3");
        let _autostart = EnvVarGuard::set("QUEDO_AUTOSTART_ENABLED", "1");
        let _log = EnvVarGuard::set("QUEDO_LOG_LEVEL", "debug");
        let _max = EnvVarGuard::set("QUEDO_MAX_RECORDING_SECONDS", "123");

        let mut config = AppConfig::default();
        apply_env_overrides(&mut config);
        assert_eq!(config.transcription.backend, BackendKind::WhisperCpp);
        assert_eq!(config.transcription.model_id.as_deref(), Some("m1"));
        assert_eq!(config.transcription.language.as_deref(), Some("en"));
        assert!(config.transcription.translate);
        assert!(config.transcription.diarize);
        assert_eq!(config.transcription.timeout_seconds, 77);
        assert_eq!(config.output.mode, OutputMode::Disabled);
        assert_eq!(config.hotkey.binding, "Ctrl+Alt+Q");
        assert_eq!(
            config.history.db_path.as_ref(),
            Some(&PathBuf::from("/tmp/h.sqlite3"))
        );
        assert!(config.service.autostart_enabled);
        assert_eq!(config.diagnostics.log_level, "debug");
        assert_eq!(config.audio.max_recording_seconds, 123);
    }

    #[test]
    fn cli_overrides_update_fields() {
        let mut config = AppConfig::default();
        let overrides = CliOverrides {
            backend: Some(BackendKind::InsanelyFast),
            model_id: Some("model-x".to_owned()),
            language: Some("fr".to_owned()),
            timeout_seconds: Some(66),
            diarize: Some(true),
            translate: Some(true),
            hotkey_binding: Some("Ctrl+Shift+R".to_owned()),
            output_mode: Some(OutputMode::Disabled),
            ..CliOverrides::default()
        };
        apply_cli_overrides(&mut config, &overrides);
        assert_eq!(config.transcription.backend, BackendKind::InsanelyFast);
        assert_eq!(config.transcription.model_id.as_deref(), Some("model-x"));
        assert_eq!(config.transcription.language.as_deref(), Some("fr"));
        assert_eq!(config.transcription.timeout_seconds, 66);
        assert!(config.transcription.diarize);
        assert!(config.transcription.translate);
        assert_eq!(config.hotkey.binding, "Ctrl+Shift+R");
        assert_eq!(config.output.mode, OutputMode::Disabled);
    }
}

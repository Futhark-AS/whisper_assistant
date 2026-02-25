use std::path::PathBuf;

use clap::{Parser, Subcommand};
use franken_whisper::BackendKind;

use crate::config::{CliOverrides, OutputMode};

#[derive(Debug, Parser)]
#[command(name = "quedo-daemon")]
#[command(about = "Quedo FrankenWhisper transcription daemon")]
pub struct Cli {
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, value_enum)]
    pub backend: Option<BackendKind>,

    #[arg(long)]
    pub model_id: Option<String>,

    #[arg(long)]
    pub language: Option<String>,

    #[arg(long)]
    pub timeout_seconds: Option<u64>,

    #[arg(long)]
    pub diarize: Option<bool>,

    #[arg(long)]
    pub translate: Option<bool>,

    #[arg(long)]
    pub hotkey_binding: Option<String>,

    #[arg(long)]
    pub output_mode: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run,
    Doctor {
        #[arg(long)]
        json: bool,
    },
    Install,
    Status,
}

impl Cli {
    pub fn to_overrides(&self) -> CliOverrides {
        CliOverrides {
            config_path: self.config.clone(),
            backend: self.backend,
            model_id: self.model_id.clone(),
            language: self.language.clone(),
            timeout_seconds: self.timeout_seconds,
            diarize: self.diarize,
            translate: self.translate,
            hotkey_binding: self.hotkey_binding.clone(),
            output_mode: self
                .output_mode
                .as_deref()
                .and_then(parse_output_mode_override),
        }
    }
}

fn parse_output_mode_override(raw: &str) -> Option<OutputMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "clipboard_only" | "clipboard-only" => Some(OutputMode::ClipboardOnly),
        "disabled" | "none" => Some(OutputMode::Disabled),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_output_mode_override, Cli, Command};
    use crate::config::OutputMode;
    use franken_whisper::BackendKind;
    use std::path::PathBuf;

    #[test]
    fn output_mode_aliases_parse() {
        assert_eq!(
            parse_output_mode_override("clipboard_only"),
            Some(OutputMode::ClipboardOnly)
        );
        assert_eq!(
            parse_output_mode_override("clipboard-only"),
            Some(OutputMode::ClipboardOnly)
        );
        assert_eq!(
            parse_output_mode_override("disabled"),
            Some(OutputMode::Disabled)
        );
        assert_eq!(parse_output_mode_override("none"), Some(OutputMode::Disabled));
        assert_eq!(parse_output_mode_override("unknown"), None);
    }

    #[test]
    fn to_overrides_maps_all_fields() {
        let cli = Cli {
            config: Some(PathBuf::from("/tmp/config.toml")),
            backend: Some(BackendKind::WhisperCpp),
            model_id: Some("model-a".to_owned()),
            language: Some("en".to_owned()),
            timeout_seconds: Some(88),
            diarize: Some(true),
            translate: Some(true),
            hotkey_binding: Some("Ctrl+Shift+Space".to_owned()),
            output_mode: Some("clipboard-only".to_owned()),
            command: Command::Status,
        };

        let overrides = cli.to_overrides();
        assert_eq!(overrides.config_path, Some(PathBuf::from("/tmp/config.toml")));
        assert_eq!(overrides.backend, Some(BackendKind::WhisperCpp));
        assert_eq!(overrides.model_id.as_deref(), Some("model-a"));
        assert_eq!(overrides.language.as_deref(), Some("en"));
        assert_eq!(overrides.timeout_seconds, Some(88));
        assert_eq!(overrides.diarize, Some(true));
        assert_eq!(overrides.translate, Some(true));
        assert_eq!(overrides.hotkey_binding.as_deref(), Some("Ctrl+Shift+Space"));
        assert_eq!(overrides.output_mode, Some(OutputMode::ClipboardOnly));
    }

    #[test]
    fn invalid_output_mode_does_not_override() {
        let cli = Cli {
            config: None,
            backend: None,
            model_id: None,
            language: None,
            timeout_seconds: None,
            diarize: None,
            translate: None,
            hotkey_binding: None,
            output_mode: Some("invalid".to_owned()),
            command: Command::Run,
        };

        let overrides = cli.to_overrides();
        assert!(overrides.output_mode.is_none());
    }
}

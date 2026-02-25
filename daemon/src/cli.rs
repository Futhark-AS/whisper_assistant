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

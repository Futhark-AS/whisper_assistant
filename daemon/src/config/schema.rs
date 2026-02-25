use std::path::PathBuf;

use franken_whisper::BackendKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub output: OutputConfig,
    pub history: HistoryConfig,
    pub service: ServiceConfig,
    pub diagnostics: DiagnosticsConfig,
    pub permissions: PermissionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub binding: String,
    pub retry_strategy: HotkeyRetryStrategy,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            binding: "Ctrl+Shift+Space".to_owned(),
            retry_strategy: HotkeyRetryStrategy::Immediate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyRetryStrategy {
    Immediate,
    ExponentialBackoff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub device: Option<String>,
    pub sample_rate_preference: Option<u32>,
    pub max_recording_seconds: u32,
    pub retain_audio: bool,
    pub arming_timeout_ms: u64,
    pub stall_timeout_ms: u64,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device: None,
            sample_rate_preference: None,
            max_recording_seconds: 300,
            retain_audio: false,
            arming_timeout_ms: 2_000,
            stall_timeout_ms: 750,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranscriptionConfig {
    pub backend: BackendKind,
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub translate: bool,
    pub diarize: bool,
    pub timeout_seconds: u64,
    pub threads: Option<u32>,
    pub processors: Option<u32>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            backend: BackendKind::Auto,
            model_id: None,
            language: None,
            translate: false,
            diarize: false,
            timeout_seconds: 45,
            threads: None,
            processors: None,
        }
    }
}

impl TranscriptionConfig {
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_seconds.saturating_mul(1_000)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub mode: OutputMode,
    pub enable_notifications: bool,
    pub auto_paste_delay_ms: u64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            mode: OutputMode::ClipboardOnly,
            enable_notifications: true,
            auto_paste_delay_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    ClipboardOnly,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub db_path: Option<PathBuf>,
    pub max_entries: usize,
    pub prune_policy: PrunePolicy,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            max_entries: 1_000,
            prune_policy: PrunePolicy::NoPrune,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrunePolicy {
    NoPrune,
    KeepRecent,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServiceConfig {
    pub autostart_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    pub log_level: String,
    pub log_retention_days: u32,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_owned(),
            log_retention_days: 14,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionsConfig {
    pub microphone_required: bool,
    pub accessibility_required: bool,
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            microphone_required: true,
            accessibility_required: false,
        }
    }
}

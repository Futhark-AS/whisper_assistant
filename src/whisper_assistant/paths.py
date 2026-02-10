"""
XDG-compliant paths for whisper-assistant.

Config:  ~/.config/whisper-assistant/
Data:    ~/.local/share/whisper-assistant/
State:   ~/.local/state/whisper-assistant/
"""

import os
from pathlib import Path

APP_NAME = "whisper-assistant"

DEFAULT_CONFIG = """\
# Run `whisper-assistant config edit` to edit these

GROQ_API_KEY=your_api_key

TOGGLE_RECORDING_HOTKEY=ctrl+shift+1

RETRY_TRANSCRIPTION_HOTKEY=ctrl+shift+2

CANCEL_RECORDING_HOTKEY=ctrl+shift+3

TRANSCRIPTION_LANGUAGE=auto

TRANSCRIPTION_OUTPUT=paste_on_cursor

WHISPER_MODEL=whisper-large-v3-turbo

GROQ_TIMEOUT=60

# Comma-separated words to improve transcription accuracy (e.g. Claude,Cloudgeni)
VOCABULARY=
"""


def get_config_dir() -> Path:
    """~/.config/whisper-assistant/"""
    xdg = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    return xdg / APP_NAME


def get_data_dir() -> Path:
    """~/.local/share/whisper-assistant/ - for history recordings"""
    xdg = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
    return xdg / APP_NAME


def get_state_dir() -> Path:
    """~/.local/state/whisper-assistant/ - for logs and runtime state"""
    xdg = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state"))
    return xdg / APP_NAME


def get_config_file() -> Path:
    """Get config file path, creating default if needed. Secured with chmod 600."""
    config_dir = get_config_dir()
    config_dir.mkdir(parents=True, exist_ok=True)
    config_file = config_dir / "config.env"
    if not config_file.exists():
        config_file.write_text(DEFAULT_CONFIG)
        config_file.chmod(0o600)
    return config_file


def get_history_dir() -> Path:
    """Get history directory for recordings."""
    history_dir = get_data_dir() / "history"
    history_dir.mkdir(parents=True, exist_ok=True)
    return history_dir


def get_log_dir() -> Path:
    """Get log directory."""
    log_dir = get_state_dir() / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    return log_dir


def get_pid_file() -> Path:
    """Get PID file path for daemon."""
    state_dir = get_state_dir()
    state_dir.mkdir(parents=True, exist_ok=True)
    return state_dir / "daemon.pid"

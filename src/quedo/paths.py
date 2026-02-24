"""
XDG-compliant paths for quedo.

Config:  ~/.config/quedo/
Data:    ~/.local/share/quedo/
State:   ~/.local/state/quedo/
"""

import os
import shutil
from pathlib import Path

APP_NAME = "quedo"
LEGACY_APP_NAME = "whisper-assistant"

DEFAULT_CONFIG = """\
# Run `quedo config edit` to edit these

GROQ_API_KEY=your_api_key

TOGGLE_RECORDING_HOTKEY=ctrl+shift+1

RETRY_TRANSCRIPTION_HOTKEY=ctrl+shift+2

CANCEL_RECORDING_HOTKEY=ctrl+shift+3

TRANSCRIPTION_LANGUAGE=auto

TRANSCRIPTION_OUTPUT=clipboard

WHISPER_MODEL=whisper-large-v3-turbo

GROQ_TIMEOUT=60

# Comma-separated words to improve transcription accuracy (e.g. Claude,Cloudgeni)
VOCABULARY=
"""


def get_config_dir() -> Path:
    """~/.config/quedo/"""
    xdg = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    target = xdg / APP_NAME
    legacy = xdg / LEGACY_APP_NAME
    _migrate_legacy_dir(target, legacy)
    return target


def get_data_dir() -> Path:
    """~/.local/share/quedo/ - for history recordings"""
    xdg = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share"))
    target = xdg / APP_NAME
    legacy = xdg / LEGACY_APP_NAME
    _migrate_legacy_dir(target, legacy)
    return target


def get_state_dir() -> Path:
    """~/.local/state/quedo/ - for logs and runtime state"""
    xdg = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state"))
    target = xdg / APP_NAME
    legacy = xdg / LEGACY_APP_NAME
    _migrate_legacy_dir(target, legacy)
    return target


def _migrate_legacy_dir(target: Path, legacy: Path) -> None:
    if target.exists() or not legacy.exists():
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        legacy.rename(target)
    except OSError:
        shutil.copytree(legacy, target, dirs_exist_ok=True)


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

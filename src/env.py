import os
from pathlib import Path
from dataclasses import dataclass, fields
import logging
from typing import Set, Union, Optional
from pynput import keyboard
from dotenv import load_dotenv
import shutil
import click

logger = logging.getLogger(__name__)


class ConfigError(ValueError):
    """Configuration error with key name."""

    def __init__(self, key: str, message: str):
        self.key = key
        self.message = message
        super().__init__(f"{key}: {message}")


class ConfigErrors(ValueError):
    """Multiple configuration errors."""

    def __init__(self, errors: list[ConfigError]):
        self.errors = errors
        super().__init__("\n".join(str(e) for e in errors))


@dataclass
class TranscriptionOutput:
    clipboard: bool = False
    paste_on_cursor: bool = False

    @classmethod
    def valid_options(cls) -> set[str]:
        return {f.name for f in fields(cls)} | {"none"}

    @classmethod
    def from_string(cls, output_str: Optional[str]) -> "TranscriptionOutput":
        valid = cls.valid_options()
        if not output_str:
            raise ConfigError(
                "TRANSCRIPTION_OUTPUT", f"cannot be empty. Valid: {', '.join(valid)}"
            )

        if output_str.lower() == "none":
            return cls()

        parts = {p.strip().lower() for p in output_str.split(",") if p.strip()}
        invalid = parts - valid
        if invalid:
            raise ConfigError(
                "TRANSCRIPTION_OUTPUT",
                f"invalid value '{', '.join(invalid)}'. Valid: {', '.join(valid)}",
            )
        return cls(**{opt: True for opt in parts if opt != "none"})

    def __str__(self):
        enabled = [f.name for f in fields(self) if getattr(self, f.name)]
        return ", ".join(enabled) if enabled else "none"


@dataclass
class Env:
    GROQ_API_KEY: str
    TOGGLE_RECORDING_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]]
    RETRY_TRANSCRIPTION_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]]
    CANCEL_RECORDING_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]]
    TRANSCRIPTION_LANGUAGE: Optional[str]  # None means auto-detect
    TRANSCRIPTION_OUTPUT: TranscriptionOutput

    def __str__(self):
        lang = self.TRANSCRIPTION_LANGUAGE if self.TRANSCRIPTION_LANGUAGE else "auto"
        return (
            f"{'GROQ_API_KEY:':<30} {self.GROQ_API_KEY}\n"
            f"{'TOGGLE_RECORDING_HOTKEY:':<30} {self.TOGGLE_RECORDING_HOTKEY}\n"
            f"{'RETRY_TRANSCRIPTION_HOTKEY:':<30} {self.RETRY_TRANSCRIPTION_HOTKEY}\n"
            f"{'CANCEL_RECORDING_HOTKEY:':<30} {self.CANCEL_RECORDING_HOTKEY}\n"
            f"{'TRANSCRIPTION_LANGUAGE:':<30} {lang}\n"
            f"{'TRANSCRIPTION_OUTPUT:':<30} {self.TRANSCRIPTION_OUTPUT}"
        )


VALID_MODIFIERS = ["cmd", "ctrl", "control", "shift", "alt", "option"]


def _parse_hotkey(
    key_name: str,
    hotkey_str: Optional[str],
) -> Set[Union[keyboard.Key, keyboard.KeyCode]]:
    if not hotkey_str:
        raise ConfigError(
            key_name,
            f"cannot be empty. Format: modifier+key (e.g. cmd+shift+r). Modifiers: {', '.join(VALID_MODIFIERS)}",
        )

    parts = [p.strip().lower() for p in hotkey_str.split("+")]
    if not parts:
        raise ConfigError(
            key_name,
            f"invalid format '{hotkey_str}'. Format: modifier+key (e.g. cmd+shift+r)",
        )

    keys = set()
    key_map = {
        "cmd": keyboard.Key.cmd,
        "ctrl": keyboard.Key.ctrl,
        "control": keyboard.Key.ctrl,
        "shift": keyboard.Key.shift,
        "alt": keyboard.Key.alt,
        "option": keyboard.Key.alt,
    }

    # Process modifiers and the final key
    for part in parts:
        if part in key_map:
            keys.add(key_map[part])
        elif len(part) == 1:
            # Single character key
            keys.add(keyboard.KeyCode.from_char(part))
        else:
            # Try to find as a Key attribute (e.g., 'space', 'enter')
            try:
                key_attr = getattr(keyboard.Key, part)
                keys.add(key_attr)
            except AttributeError:
                raise ConfigError(
                    key_name,
                    f"unknown key '{part}'. Modifiers: {', '.join(VALID_MODIFIERS)}",
                )

    return keys


def read_env():
    env_file = Path.cwd() / ".env"
    if not env_file.exists():
        # Copy .env.example to .env
        shutil.copy(Path.cwd() / ".env.example", env_file)
        logger.info("No .env file found. Copied .env.example to .env")

    load_dotenv(override=True)

    errors: list[ConfigError] = []

    GROQ_API_KEY = os.getenv("GROQ_API_KEY")
    if not GROQ_API_KEY:
        errors.append(ConfigError("GROQ_API_KEY", "cannot be empty"))

    TOGGLE_RECORDING_HOTKEY = None
    try:
        TOGGLE_RECORDING_HOTKEY = _parse_hotkey(
            "TOGGLE_RECORDING_HOTKEY", os.getenv("TOGGLE_RECORDING_HOTKEY")
        )
    except ConfigError as e:
        errors.append(e)

    RETRY_TRANSCRIPTION_HOTKEY = None
    try:
        RETRY_TRANSCRIPTION_HOTKEY = _parse_hotkey(
            "RETRY_TRANSCRIPTION_HOTKEY", os.getenv("RETRY_TRANSCRIPTION_HOTKEY")
        )
    except ConfigError as e:
        errors.append(e)

    CANCEL_RECORDING_HOTKEY = None
    try:
        CANCEL_RECORDING_HOTKEY = _parse_hotkey(
            "CANCEL_RECORDING_HOTKEY", os.getenv("CANCEL_RECORDING_HOTKEY")
        )
    except ConfigError as e:
        errors.append(e)

    TRANSCRIPTION_LANGUAGE = os.getenv("TRANSCRIPTION_LANGUAGE")
    if not TRANSCRIPTION_LANGUAGE:
        errors.append(
            ConfigError(
                "TRANSCRIPTION_LANGUAGE",
                "cannot be empty. Use 'auto' for auto-detection or a language code (e.g. 'en', 'es')",
            )
        )
    elif TRANSCRIPTION_LANGUAGE.lower() == "auto":
        # Normalize "auto" to None for internal use
        TRANSCRIPTION_LANGUAGE = None

    TRANSCRIPTION_OUTPUT = None
    try:
        TRANSCRIPTION_OUTPUT = TranscriptionOutput.from_string(
            os.getenv("TRANSCRIPTION_OUTPUT")
        )
    except ConfigError as e:
        errors.append(e)

    if errors:
        raise ConfigErrors(errors)

    # These assertions satisfy the type checker - we know they're not None if no errors
    assert GROQ_API_KEY is not None
    assert TOGGLE_RECORDING_HOTKEY is not None
    assert RETRY_TRANSCRIPTION_HOTKEY is not None
    assert CANCEL_RECORDING_HOTKEY is not None
    assert TRANSCRIPTION_OUTPUT is not None

    return Env(
        GROQ_API_KEY,
        TOGGLE_RECORDING_HOTKEY,
        RETRY_TRANSCRIPTION_HOTKEY,
        CANCEL_RECORDING_HOTKEY,
        TRANSCRIPTION_LANGUAGE,
        TRANSCRIPTION_OUTPUT,
    )

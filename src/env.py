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


@dataclass
class TranscriptionOutput:
    clipboard: bool = False
    paste_on_cursor: bool = False

    @classmethod
    def valid_options(cls) -> set[str]:
        return {f.name for f in fields(cls)}

    @classmethod
    def from_string(cls, output_str: Optional[str]) -> "TranscriptionOutput":
        if not output_str:
            raise ValueError("TRANSCRIPTION_OUTPUT cannot be empty")

        if output_str.lower() == "none":
            return cls()

        parts = {p.strip().lower() for p in output_str.split(",") if p.strip()}
        invalid = parts - cls.valid_options()
        if invalid:
            raise ValueError(
                f"Invalid transcription output mode(s): {invalid}. "
                f"Valid options: {cls.valid_options()}"
            )
        return cls(**{opt: True for opt in parts})

    def __str__(self):
        enabled = [f.name for f in fields(self) if getattr(self, f.name)]
        return ", ".join(enabled) if enabled else "none"


@dataclass
class Env:
    GROQ_API_KEY: Optional[str]
    TOGGLE_RECORDING_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]]
    RETRY_TRANSCRIPTION_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]]
    TRANSCRIPTION_LANGUAGE: Optional[str]
    TRANSCRIPTION_OUTPUT: TranscriptionOutput

    def __str__(self):
        return (
            f"{'GROQ_API_KEY:':<30} {self.GROQ_API_KEY}\n"
            f"{'TOGGLE_RECORDING_HOTKEY:':<30} {self.TOGGLE_RECORDING_HOTKEY}\n"
            f"{'RETRY_TRANSCRIPTION_HOTKEY:':<30} {self.RETRY_TRANSCRIPTION_HOTKEY}\n"
            f"{'TRANSCRIPTION_LANGUAGE:':<30} {self.TRANSCRIPTION_LANGUAGE or 'auto detect'}\n"
            f"{'TRANSCRIPTION_OUTPUT:':<30} {self.TRANSCRIPTION_OUTPUT}"
        )


def _parse_hotkey(
    hotkey_str: Optional[str],
) -> Set[Union[keyboard.Key, keyboard.KeyCode]]:
    if not hotkey_str:
        raise ValueError("Hotkey string cannot be empty")

    parts = [p.strip().lower() for p in hotkey_str.split("+")]
    if not parts:
        raise ValueError(f"Invalid hotkey format: {hotkey_str}")

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
                raise ValueError(
                    f"Unknown key or modifier: {part} in hotkey '{hotkey_str}'"
                )

    return keys


def read_env():
    env_file = Path.cwd() / ".env"
    if not env_file.exists():
        # Copy .env.example to .env
        shutil.copy(Path.cwd() / ".env.example", env_file)
        logger.info("No .env file found. Copied .env.example to .env")

    load_dotenv(override=True)

    GROQ_API_KEY: Optional[str] = os.getenv("GROQ_API_KEY")
    if not GROQ_API_KEY:
        raise ValueError("GROQ_API_KEY is not set")

    TOGGLE_RECORDING_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]] = _parse_hotkey(
        os.getenv("TOGGLE_RECORDING_HOTKEY")
    )
    RETRY_TRANSCRIPTION_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]] = (
        _parse_hotkey(os.getenv("RETRY_TRANSCRIPTION_HOTKEY"))
    )
    TRANSCRIPTION_LANGUAGE: Optional[str] = os.getenv("TRANSCRIPTION_LANGUAGE")
    TRANSCRIPTION_OUTPUT = TranscriptionOutput.from_string(
        os.getenv("TRANSCRIPTION_OUTPUT")
    )

    return Env(
        GROQ_API_KEY,
        TOGGLE_RECORDING_HOTKEY,
        RETRY_TRANSCRIPTION_HOTKEY,
        TRANSCRIPTION_LANGUAGE,
        TRANSCRIPTION_OUTPUT,
    )

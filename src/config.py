import os
import logging
from typing import Set, Union, Optional
from pynput import keyboard
from dotenv import load_dotenv

logger = logging.getLogger(__name__)


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


# Load environment variables from .env file
load_dotenv()

GROQ_API_KEY: Optional[str] = os.getenv("GROQ_API_KEY")
if not GROQ_API_KEY:
    raise ValueError("GROQ_API_KEY is not set")

TOGGLE_RECORDING_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]] = _parse_hotkey(
    os.getenv("TOGGLE_RECORDING_HOTKEY")
)
RETRY_TRANSCRIPTION_HOTKEY: Set[Union[keyboard.Key, keyboard.KeyCode]] = _parse_hotkey(
    os.getenv("RETRY_TRANSCRIPTION_HOTKEY")
)
TRANSCRIPTION_LANGUAGE: Optional[str] = os.getenv("TRANSCRIPTION_LANGUAGE")

PERSIST_ONLY_LATEST: bool = os.getenv("PERSIST_ONLY_LATEST", "false").lower() == "true"

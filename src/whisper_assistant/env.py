import logging
import os
from dataclasses import dataclass, fields

from dotenv import load_dotenv
from pynput import keyboard

from whisper_assistant.paths import get_config_file

logger = logging.getLogger(__name__)

VALID_WHISPER_MODELS = {
    "whisper-large-v3",
    "whisper-large-v3-turbo",
    "distil-whisper-large-v3-en",
}


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
    def from_string(cls, output_str: str | None) -> "TranscriptionOutput":
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

    def __str__(self) -> str:
        enabled = [f.name for f in fields(self) if getattr(self, f.name)]
        return ", ".join(enabled) if enabled else "none"


@dataclass
class Env:
    GROQ_API_KEY: str
    TOGGLE_RECORDING_HOTKEY: set[keyboard.Key | keyboard.KeyCode]
    RETRY_TRANSCRIPTION_HOTKEY: set[keyboard.Key | keyboard.KeyCode]
    CANCEL_RECORDING_HOTKEY: set[keyboard.Key | keyboard.KeyCode]
    TRANSCRIPTION_LANGUAGE: str | None  # None means auto-detect
    TRANSCRIPTION_OUTPUT: TranscriptionOutput
    WHISPER_MODEL: str
    GROQ_TIMEOUT: int
    VOCABULARY: list[str]  # Words to bias transcription toward

    def _masked_key(self) -> str:
        if len(self.GROQ_API_KEY) >= 4:
            return "****" + self.GROQ_API_KEY[-4:]
        return "****"

    def __str__(self) -> str:
        lang = self.TRANSCRIPTION_LANGUAGE if self.TRANSCRIPTION_LANGUAGE else "auto"
        return (
            f"{'GROQ_API_KEY:':<30} {self._masked_key()}\n"
            f"{'TOGGLE_RECORDING_HOTKEY:':<30} {self.TOGGLE_RECORDING_HOTKEY}\n"
            f"{'RETRY_TRANSCRIPTION_HOTKEY:':<30} {self.RETRY_TRANSCRIPTION_HOTKEY}\n"
            f"{'CANCEL_RECORDING_HOTKEY:':<30} {self.CANCEL_RECORDING_HOTKEY}\n"
            f"{'TRANSCRIPTION_LANGUAGE:':<30} {lang}\n"
            f"{'TRANSCRIPTION_OUTPUT:':<30} {self.TRANSCRIPTION_OUTPUT}\n"
            f"{'WHISPER_MODEL:':<30} {self.WHISPER_MODEL}\n"
            f"{'GROQ_TIMEOUT:':<30} {self.GROQ_TIMEOUT}\n"
            f"{'VOCABULARY:':<30} {', '.join(self.VOCABULARY) if self.VOCABULARY else '(none)'}"
        )


VALID_MODIFIERS = ["cmd", "ctrl", "control", "shift", "alt", "option"]


def _parse_hotkey(
    key_name: str,
    hotkey_str: str | None,
) -> set[keyboard.Key | keyboard.KeyCode]:
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

    keys: set[keyboard.Key | keyboard.KeyCode] = set()
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


def read_env() -> Env:
    """Read and validate config from env file, returning an Env instance."""
    config_file = get_config_file()
    load_dotenv(config_file, override=True)

    errors: list[ConfigError] = []

    # --- Simple string fields ---
    GROQ_API_KEY = os.getenv("GROQ_API_KEY")
    if not GROQ_API_KEY:
        errors.append(ConfigError("GROQ_API_KEY", "cannot be empty"))

    # --- Hotkeys (share try/except pattern) ---
    hotkey_fields = [
        "TOGGLE_RECORDING_HOTKEY",
        "RETRY_TRANSCRIPTION_HOTKEY",
        "CANCEL_RECORDING_HOTKEY",
    ]
    hotkeys: dict[str, set[keyboard.Key | keyboard.KeyCode] | None] = {}
    for name in hotkey_fields:
        try:
            hotkeys[name] = _parse_hotkey(name, os.getenv(name))
        except ConfigError as e:
            errors.append(e)
            hotkeys[name] = None

    # --- Transcription language ---
    TRANSCRIPTION_LANGUAGE = os.getenv("TRANSCRIPTION_LANGUAGE")
    if not TRANSCRIPTION_LANGUAGE:
        errors.append(
            ConfigError(
                "TRANSCRIPTION_LANGUAGE",
                "cannot be empty. Use 'auto' for auto-detection or a language code (e.g. 'en', 'es')",
            )
        )
    elif TRANSCRIPTION_LANGUAGE.lower() == "auto":
        TRANSCRIPTION_LANGUAGE = None

    # --- Transcription output ---
    TRANSCRIPTION_OUTPUT = None
    try:
        TRANSCRIPTION_OUTPUT = TranscriptionOutput.from_string(
            os.getenv("TRANSCRIPTION_OUTPUT")
        )
    except ConfigError as e:
        errors.append(e)

    # --- Whisper model ---
    WHISPER_MODEL = os.getenv("WHISPER_MODEL", "whisper-large-v3")
    if WHISPER_MODEL not in VALID_WHISPER_MODELS:
        errors.append(
            ConfigError(
                "WHISPER_MODEL",
                f"invalid model '{WHISPER_MODEL}'. Valid: {', '.join(sorted(VALID_WHISPER_MODELS))}",
            )
        )

    # --- Groq timeout ---
    groq_timeout_str = os.getenv("GROQ_TIMEOUT", "60")
    GROQ_TIMEOUT = 60
    try:
        GROQ_TIMEOUT = int(groq_timeout_str)
        if GROQ_TIMEOUT <= 0:
            errors.append(ConfigError("GROQ_TIMEOUT", "must be a positive integer"))
    except ValueError:
        errors.append(
            ConfigError("GROQ_TIMEOUT", f"invalid integer '{groq_timeout_str}'")
        )

    # --- Vocabulary hints ---
    vocabulary_raw = os.getenv("VOCABULARY", "")
    VOCABULARY = [w.strip() for w in vocabulary_raw.split(",") if w.strip()]

    if errors:
        raise ConfigErrors(errors)

    # These assertions satisfy the type checker - we know they're not None if no errors
    assert GROQ_API_KEY is not None
    assert hotkeys["TOGGLE_RECORDING_HOTKEY"] is not None
    assert hotkeys["RETRY_TRANSCRIPTION_HOTKEY"] is not None
    assert hotkeys["CANCEL_RECORDING_HOTKEY"] is not None
    assert TRANSCRIPTION_OUTPUT is not None

    return Env(
        GROQ_API_KEY=GROQ_API_KEY,
        TOGGLE_RECORDING_HOTKEY=hotkeys["TOGGLE_RECORDING_HOTKEY"],
        RETRY_TRANSCRIPTION_HOTKEY=hotkeys["RETRY_TRANSCRIPTION_HOTKEY"],
        CANCEL_RECORDING_HOTKEY=hotkeys["CANCEL_RECORDING_HOTKEY"],
        TRANSCRIPTION_LANGUAGE=TRANSCRIPTION_LANGUAGE,
        TRANSCRIPTION_OUTPUT=TRANSCRIPTION_OUTPUT,
        WHISPER_MODEL=WHISPER_MODEL,
        GROQ_TIMEOUT=GROQ_TIMEOUT,
        VOCABULARY=VOCABULARY,
    )

import logging
import subprocess
import threading
from datetime import datetime

import numpy as np
import soundfile as sf
from pynput.keyboard import Controller, Key, KeyCode

from whisper_assistant import log_config  # noqa: F401 — logging side-effect
from whisper_assistant.env import read_env
from whisper_assistant.packages.audio_recorder import AudioRecorder
from whisper_assistant.packages.keyboard_listener import KeyboardListener
from whisper_assistant.packages.notifications import Notifier
from whisper_assistant.packages.transcriber import Transcriber
from whisper_assistant.paths import get_history_dir

logger = logging.getLogger(__name__)

HISTORY_DIR = get_history_dir()

# macOS system sounds
SOUND_HERO = "/System/Library/Sounds/Hero.aiff"
SOUND_MORSE = "/System/Library/Sounds/Morse.aiff"
SOUND_GLASS = "/System/Library/Sounds/Glass.aiff"
SOUND_BASSO = "/System/Library/Sounds/Basso.aiff"


class WhisperApp:
    """Main application orchestrating keyboard listener, audio recorder, and transcriber."""

    def __init__(self) -> None:
        self.env = read_env()
        logger.info(f"Config:\n{self.env}")

        self.notifier = Notifier()
        self.recorder = AudioRecorder()
        self.transcriber = Transcriber(
            model=self.env.WHISPER_MODEL,
            timeout=self.env.GROQ_TIMEOUT,
        )
        self.keyboard_listener = KeyboardListener()
        self._keyboard_controller = Controller()
        self._recording_lock = threading.Lock()
        self.last_recording_data: tuple[np.ndarray, int] | None = None

    # ── Public API (hotkey callbacks) ────────────────────────────────

    def toggle_recording(self) -> None:
        """Callback for toggle recording hotkey."""
        if self.recorder.is_recording():
            logger.info("Stopping recording...")
            self.recorder.stop_recording()
            return

        if not self._recording_lock.acquire(blocking=False):
            logger.debug("Toggle ignored: recording flow already running")
            return

        logger.info("Starting recording process...")
        thread = threading.Thread(target=self._recording_flow, daemon=True)
        thread.start()

    def cancel_recording(self) -> None:
        """Cancel the current recording without transcribing."""
        if not self.recorder.is_recording():
            logger.debug("Cancel ignored: not recording")
            return

        logger.info("Cancelling recording...")
        self.recorder.cancel_recording()
        self.notifier.notify_info("Recording cancelled", SOUND_BASSO)

    def retry_transcription(self) -> None:
        """Re-transcribe the last recorded audio."""
        if self.last_recording_data is None:
            logger.warning("No previous recording to retry")
            return

        audio_data, sample_rate = self.last_recording_data
        logger.info("Retrying transcription of last recording")
        self.notifier.notify_info("Retrying transcription...", SOUND_MORSE)
        threading.Thread(
            target=self._transcribe_and_output,
            args=(audio_data, sample_rate),
            daemon=True,
        ).start()

    # ── Recording flow (runs in worker thread) ───────────────────────

    def _recording_flow(self) -> None:
        """Record → transcribe → output. Runs in a worker thread."""
        try:
            # Notify user recording is starting (async — doesn't block stream open)
            self.notifier.notify_info("Recording...", SOUND_HERO)

            result = self.recorder.start_recording()

            if result is None:
                logger.info("Recording returned no data (cancelled or error)")
                return

            audio_data, sample_rate = result
            self.last_recording_data = result

            # Immediate feedback: "processing..."
            self.notifier.notify_info("Processing...", SOUND_MORSE)

            self._transcribe_and_output(audio_data, sample_rate)

        except Exception:
            logger.exception("Recording flow error")
            self.notifier.notify_error("Recording failed — see logs")
        finally:
            self._recording_lock.release()

    def _transcribe_and_output(self, audio_data: np.ndarray, sample_rate: int) -> None:
        """Transcribe audio array and output the result."""
        try:
            vocab_prompt = ", ".join(self.env.VOCABULARY) if self.env.VOCABULARY else ""
            text = self.transcriber.transcribe_from_array(
                audio_data,
                sample_rate,
                prompt=vocab_prompt,
                language=self.env.TRANSCRIPTION_LANGUAGE,
            )

            if not text or not text.strip():
                logger.info("Transcription returned empty result")
                self.notifier.notify_info("No speech detected")
                return

            self._output_transcription(text)
            self.notifier.notify_info("Transcription complete", SOUND_GLASS)

            # Save history in background — never blocks user
            self._save_to_history_async(audio_data, sample_rate, text)

        except Exception:
            logger.exception("Transcription error")
            self.notifier.notify_error("Transcription failed — see logs")

    # ── Output ───────────────────────────────────────────────────────

    def _output_transcription(self, text: str) -> None:
        """Copy to clipboard and optionally paste at cursor."""
        output = self.env.TRANSCRIPTION_OUTPUT

        if output.clipboard or output.paste_on_cursor:
            # Copy to clipboard via pbcopy (always needed for Cmd+V paste too)
            try:
                subprocess.run(
                    ["pbcopy"],
                    input=text.encode(),
                    check=True,
                )
                logger.info("Transcription copied to clipboard")
            except Exception:
                logger.exception("Failed to copy to clipboard")
                return

        if output.paste_on_cursor:
            try:
                # Simulate Cmd+V — instant regardless of text length
                with self._keyboard_controller.pressed(Key.cmd):
                    self._keyboard_controller.tap(KeyCode.from_char("v"))
                logger.info("Transcription pasted at cursor via Cmd+V")
            except Exception:
                logger.exception("Failed to paste at cursor")

    # ── History persistence (fire-and-forget) ────────────────────────

    def _save_to_history_async(
        self, audio_data: np.ndarray, sample_rate: int, text: str
    ) -> None:
        """Spawn a daemon thread to save recording + transcription to history."""
        threading.Thread(
            target=self._save_to_history,
            args=(audio_data, sample_rate, text),
            daemon=True,
        ).start()

    def _save_to_history(
        self, audio_data: np.ndarray, sample_rate: int, text: str
    ) -> None:
        """Save audio as FLAC and transcription text to history directory."""
        try:
            today = datetime.now().strftime("%Y-%m-%d")
            timestamp = datetime.now().strftime("%H%M%S")
            entry_dir = HISTORY_DIR / today / timestamp
            entry_dir.mkdir(parents=True, exist_ok=True)

            # Save audio as FLAC (~4x smaller than WAV)
            audio_path = entry_dir / "recording.flac"
            sf.write(str(audio_path), audio_data, sample_rate, format="flac")

            # Save transcription text
            text_path = entry_dir / "transcription.txt"
            text_path.write_text(text, encoding="utf-8")

            logger.debug(f"History saved to {entry_dir}")
        except Exception:
            logger.exception("Failed to save history")

    # ── Lifecycle ────────────────────────────────────────────────────

    def run(self) -> None:
        """Start the application."""
        logger.info("Starting Whisper Assistant...")

        self.keyboard_listener.register_hotkey(
            self.env.TOGGLE_RECORDING_HOTKEY, self.toggle_recording
        )
        self.keyboard_listener.register_hotkey(
            self.env.RETRY_TRANSCRIPTION_HOTKEY, self.retry_transcription
        )
        self.keyboard_listener.register_hotkey(
            self.env.CANCEL_RECORDING_HOTKEY, self.cancel_recording
        )

        self.keyboard_listener.start()
        try:
            self.keyboard_listener.join()
        except KeyboardInterrupt:
            logger.info("Shutting down...")
            self.shutdown()

    def shutdown(self) -> None:
        """Clean shutdown of all components."""
        if self.recorder.is_recording():
            self.recorder.stop_recording()
        self.keyboard_listener.stop()


if __name__ == "__main__":
    app = WhisperApp()
    app.run()

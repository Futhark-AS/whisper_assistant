import logging
import threading
from pathlib import Path
from datetime import datetime
from env import read_env
import log_config  # Import logging configuration
from packages.audio_recorder import AudioRecorder
from packages.transcriber import Transcriber
from packages.keyboard_listener import KeyboardListener
from packages.notifications import Notifier

logger = logging.getLogger(__name__)

HISTORY_DIR = Path("history")


class WhisperApp:
    """Main application orchestrating keyboard listener, audio recorder, and transcriber."""

    def __init__(self):
        """Initialize the application components."""
        self.notifier = Notifier()
        self.recorder = AudioRecorder(notifier=self.notifier)
        self.transcriber = Transcriber()
        self.keyboard_listener = KeyboardListener()
        self.last_audio_file = None
        self.env = read_env()

    def stop_recording(self):
        """Stop the current recording."""
        if not self.recorder.is_recording():
            logger.debug("Stop command ignored: Not recording")
            return

        logger.info("Stopping recording...")
        self.recorder.stop_recording()

    def toggle_recording(self):
        """Callback for toggle recording hotkey."""
        if self.recorder.is_recording():
            self.stop_recording()
        else:
            logger.info("Starting recording process...")
            # Run recording in a separate thread to avoid blocking the keyboard listener
            threading.Thread(target=self._recording_flow).start()

    def retry_transcription(self):
        """Callback for retry transcription hotkey."""
        if self.last_audio_file:
            logger.info(f"Retrying transcription of: {self.last_audio_file}")
            self.transcribe_file(
                self.last_audio_file, language=self.env.TRANSCRIPTION_LANGUAGE
            )
        else:
            logger.warning("No previous audio file to retry transcription")

    def _get_today_history_dir(self):
        """Get or create today's history directory."""
        today = datetime.now().strftime("%Y-%m-%d")
        history_path = HISTORY_DIR / today
        history_path.mkdir(parents=True, exist_ok=True)
        return history_path

    def _recording_flow(self):
        """
        Handles the recording process: record -> save -> transcribe.
        This runs in a separate thread.
        """
        # Get today's history directory
        history_dir = self._get_today_history_dir()

        # Generate timestamp folder name (HHMMSS format, no separators)
        timestamp = datetime.now().strftime("%H%M%S")
        entry_dir = history_dir / timestamp
        entry_dir.mkdir(parents=True, exist_ok=True)

        # Audio file path: history/YYYY-MM-DD/HHMMSS/recording.wav
        audio_path = entry_dir / "recording.wav"

        # This blocks until stop_recording() is called
        # The AudioRecorder will notify when recording actually starts
        file_path = self.recorder.start_recording(
            output_path=str(audio_path),
            notification_message=f"Recording with lang={self.env.TRANSCRIPTION_LANGUAGE}",
        )

        if file_path:
            self.last_audio_file = file_path
            self.transcribe_file(file_path, language=self.env.TRANSCRIPTION_LANGUAGE)
        else:
            logger.error("Recording completed but no file path was returned")

    def transcribe_file(self, audio_file_path, language):
        """Transcribe audio file and output the result."""
        try:
            text = self.transcriber.transcribe(audio_file_path, language=language)

            if text:
                self._print_and_copy_transcription(text)
                # Save transcription to file
                self._save_transcription(audio_file_path, text)
                # Notify user that transcription is complete
                self.notify_completion()
            else:
                logger.info("Transcription returned empty result")

        except Exception as e:
            logger.error(f"Error during transcription: {e}", exc_info=True)

    def _save_transcription(self, audio_file_path, text):
        """Save transcription text to a file alongside the audio file."""
        try:
            # Save as transcription.txt in the same directory as recording.wav
            audio_path = Path(audio_file_path)
            transcription_path = audio_path.parent / "transcription.txt"
            with open(transcription_path, "w", encoding="utf-8") as f:
                f.write(text)
            logger.debug(f"Transcription saved to {transcription_path}")
        except Exception as e:
            logger.error(f"Error saving transcription: {e}", exc_info=True)

    def _print_and_copy_transcription(self, text):
        """Helper to print transcription and copy to clipboard."""
        print(f"\n{'=' * 60}")
        print(f"Transcription:")
        print(f"{text}")
        print(f"{'=' * 60}\n")

        try:
            import pyperclip

            pyperclip.copy(text)
            logger.info("Transcription copied to clipboard")
        except ImportError:
            logger.debug("pyperclip not available, skipping clipboard copy")

    def notify_completion(self):
        self.notifier.show_alert("Transcription complete", "Whisper Assistant")
        self.notifier.play_sound("/System/Library/Sounds/Glass.aiff", volume=25)

    def run(self):
        """Start the application."""
        logger.info("Starting Whisper Assistant...")
        logger.info(f"Config: {self.env.__dict__}")

        # Register hotkeys
        self.keyboard_listener.register_hotkey(
            self.env.TOGGLE_RECORDING_HOTKEY, self.toggle_recording
        )
        self.keyboard_listener.register_hotkey(
            self.env.RETRY_TRANSCRIPTION_HOTKEY, self.retry_transcription
        )

        # Start listener and block main thread
        self.keyboard_listener.start()
        try:
            self.keyboard_listener.join()
        except KeyboardInterrupt:
            logger.info("Shutting down...")
            self.shutdown()

    def shutdown(self):
        """Clean shutdown of all components."""
        if self.recorder.is_recording():
            self.recorder.stop_recording()
        self.keyboard_listener.stop()


if __name__ == "__main__":
    app = WhisperApp()
    app.run()

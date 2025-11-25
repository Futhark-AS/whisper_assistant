import logging
import threading
import config
import log_config  # Import logging configuration
from packages.audio_recorder import AudioRecorder
from packages.transcriber import Transcriber
from packages.keyboard_listener import KeyboardListener

logger = logging.getLogger(__name__)


class WhisperApp:
    """Main application orchestrating keyboard listener, audio recorder, and transcriber."""

    def __init__(self):
        """Initialize the application components."""
        self.recorder = AudioRecorder()
        self.transcriber = Transcriber()
        self.keyboard_listener = KeyboardListener()
        self.last_audio_file = None

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
            self.transcribe_file(self.last_audio_file)
        else:
            logger.warning("No previous audio file to retry transcription")

    def _recording_flow(self):
        """
        Handles the recording process: record -> save -> transcribe.
        This runs in a separate thread.
        """
        # This blocks until stop_recording() is called
        file_path = self.recorder.start_recording()

        if file_path:
            # Cleanup old recordings if configured
            if config.PERSIST_ONLY_LATEST:
                self.recorder.cleanup_old_recordings(file_path)

            self.last_audio_file = file_path
            self.transcribe_file(file_path)
        else:
            logger.error("Recording completed but no file path was returned")

    def transcribe_file(self, audio_file_path):
        """Transcribe audio file and output the result."""
        try:
            text = self.transcriber.transcribe(
                audio_file_path, language=config.TRANSCRIPTION_LANGUAGE
            )

            if text:
                self._print_and_copy_transcription(text)
            else:
                logger.info("Transcription returned empty result")

        except Exception as e:
            logger.error(f"Error during transcription: {e}", exc_info=True)

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

    def run(self):
        """Start the application."""
        logger.info("Starting Whisper Assistant...")
        logger.info(f"Press {config.TOGGLE_RECORDING_HOTKEY} to toggle recording")
        logger.info(f"Press {config.RETRY_TRANSCRIPTION_HOTKEY} to retry transcription")

        # Register hotkeys
        self.keyboard_listener.register_hotkey(
            config.TOGGLE_RECORDING_HOTKEY, self.toggle_recording
        )
        self.keyboard_listener.register_hotkey(
            config.RETRY_TRANSCRIPTION_HOTKEY, self.retry_transcription
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


def main():
    """Entry point."""
    app = WhisperApp()
    app.run()


if __name__ == "__main__":
    main()

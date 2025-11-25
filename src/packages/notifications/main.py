import logging
import subprocess
import platform

logger = logging.getLogger(__name__)


class Notifier:
    """Handles desktop notifications and sound feedback."""

    def _play_sound(self, sound_file, volume=25):
        """
        Play a system sound at a specified volume.

        Args:
            sound_file: Path to the sound file
            volume: Volume level (0-100)
        """
        if platform.system() == "Darwin":  # macOS
            try:
                subprocess.run(
                    [
                        "osascript",
                        "-e",
                        f"""set currentVol to output volume of (get volume settings)
set volume output volume {volume}
do shell script "afplay {sound_file}"
set volume output volume currentVol""",
                    ],
                    check=False,
                    capture_output=True,
                )
            except Exception as e:
                logger.debug(f"Error playing sound: {e}")

    def notify_recording_start(self):
        """Play sound when recording starts."""
        self._play_sound("/System/Library/Sounds/Glass.aiff", volume=25)

    def notify_completion(
        self, message="Transcription complete", title="Whisper Assistant"
    ):
        """
        Send desktop notification and play sound when transcription completes.

        Args:
            message: Notification message text
            title: Notification title
        """
        if platform.system() == "Darwin":  # macOS
            try:
                # Desktop notification using osascript
                subprocess.run(
                    [
                        "osascript",
                        "-e",
                        f'display notification "{message}" with title "{title}"',
                    ],
                    check=False,
                    capture_output=True,
                )
                # Play system sound at lower volume
                self._play_sound("/System/Library/Sounds/Hero.aiff", volume=25)
            except Exception as e:
                logger.debug(f"Error sending notification: {e}")

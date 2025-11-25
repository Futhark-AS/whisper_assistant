import logging
import subprocess
import platform

logger = logging.getLogger(__name__)


class Notifier:
    """Handles desktop notifications and sound feedback."""

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
                # Get current volume, set to 25%, play sound, then restore
                subprocess.run(
                    [
                        "osascript",
                        "-e",
                        """set currentVol to output volume of (get volume settings)
set volume output volume 25
do shell script "afplay /System/Library/Sounds/Hero.aiff"
set volume output volume currentVol""",
                    ],
                    check=False,
                    capture_output=True,
                )
            except Exception as e:
                logger.debug(f"Error sending notification: {e}")

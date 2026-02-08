import logging
import platform
import subprocess
import threading

logger = logging.getLogger(__name__)


class Notifier:
    """Handles desktop notifications and sound feedback (macOS only)."""

    def __init__(self) -> None:
        if platform.system() != "Darwin":
            raise NotImplementedError(
                f"Only macOS is supported, but got {platform.system()}"
            )

    def show_alert(self, message: str, title: str = "Whisper Assistant") -> None:
        """Show a desktop notification (non-blocking, runs in daemon thread)."""

        def _run() -> None:
            try:
                subprocess.run(
                    ["osascript", "-e", f'display notification "{message}" with title "{title}"'],
                    check=False,
                    capture_output=True,
                )
            except Exception:
                logger.exception("Failed to show alert")

        threading.Thread(target=_run, daemon=True).start()

    def play_sound(self, sound_file: str, volume: float = 0.25) -> None:
        """Play a sound file at given volume (0.0-1.0). Fire and forget, no system volume changes."""
        try:
            subprocess.Popen(
                ["afplay", "-v", str(volume), sound_file],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except Exception:
            logger.exception("Failed to play sound: %s", sound_file)

    def notify_error(self, message: str) -> None:
        """Show error alert and play error sound."""
        self.show_alert(message, title="Whisper Assistant — Error")
        self.play_sound("/System/Library/Sounds/Basso.aiff")

    def notify_info(self, message: str, sound_path: str | None = None) -> None:
        """Show info alert, optionally play a sound."""
        self.show_alert(message)
        if sound_path:
            self.play_sound(sound_path)

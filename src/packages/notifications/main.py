import logging
import subprocess
import platform

logger = logging.getLogger(__name__)


class Notifier:
    """Handles desktop notifications and sound feedback."""

    def __init__(self):
        if platform.system() != "Darwin":
            raise NotImplementedError(
                f"Only macOS is supported, but got {platform.system()}"
            )

    def _run_osascript(self, command):
        """
        Run an osascript command.
        """
        subprocess.run(
            ["osascript", "-e", command],
            check=False,
            capture_output=True,
        )

    def show_alert(self, message, title="Whisper Assistant"):
        """
        Show a desktop notification with a specified message and title.
        """
        self._run_osascript(f'display notification "{message}" with title "{title}"')

    def play_sound(self, sound_file, volume=25):
        """
        Play a system sound at a specified volume.

        Args:
            sound_file: Path to the sound file
            volume: Volume level (0-100)
        """
        self._run_osascript(f"""set currentVol to output volume of (get volume settings)
set volume output volume {volume}
do shell script "afplay {sound_file}"
set volume output volume currentVol""")

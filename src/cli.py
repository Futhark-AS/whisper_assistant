import os
import sys
import signal
import click
import subprocess
import time
from pathlib import Path
import config
from packages.transcriber import Transcriber

# Constants
HISTORY_DIR = Path("history")
PID_FILE = Path("whisper_assistant.pid")


def is_running():
    """Check if the daemon is running."""
    if not PID_FILE.exists():
        return False

    try:
        pid = int(PID_FILE.read_text().strip())
        # Check if process is alive
        os.kill(pid, 0)
        return True
    except (ValueError, ProcessLookupError, OSError):
        # PID file exists but process is dead
        PID_FILE.unlink(missing_ok=True)
        return False


def get_pid():
    """Get the PID from the PID file."""
    if not PID_FILE.exists():
        return None
    try:
        return int(PID_FILE.read_text().strip())
    except (ValueError, FileNotFoundError):
        return None


@click.group()
def cli():
    """Whisper Assistant CLI."""
    pass


@cli.command()
def start():
    """Start the Whisper Assistant daemon."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is already running (PID: {pid})")
        sys.exit(1)

    # Get the path to the main.py script
    # Assuming cli.py is in src/, main.py is also in src/
    current_dir = Path(__file__).parent
    script_path = current_dir / "main.py"

    # Start daemon process
    try:
        # Use sys.executable to ensure we use the same Python interpreter
        process = subprocess.Popen(
            [sys.executable, str(script_path)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,  # Detach from parent process
        )

        # Write PID file
        PID_FILE.write_text(str(process.pid))
        click.echo(f"Whisper Assistant started (PID: {process.pid})")
    except Exception as e:
        click.echo(f"Failed to start Whisper Assistant: {e}", err=True)
        sys.exit(1)


@cli.command()
def stop():
    """Stop the Whisper Assistant daemon."""
    if not is_running():
        click.echo("Whisper Assistant is not running")
        sys.exit(1)

    pid = get_pid()
    if pid is None:
        click.echo("Could not read PID file")
        sys.exit(1)

    try:
        # Send SIGINT to trigger graceful shutdown
        os.kill(pid, signal.SIGINT)
        # Wait a bit for graceful shutdown
        time.sleep(0.5)

        # Check if still running
        if is_running():
            # Force kill if still running
            os.kill(pid, signal.SIGTERM)
            time.sleep(0.5)

        # Clean up PID file
        PID_FILE.unlink(missing_ok=True)
        click.echo("Whisper Assistant stopped")
    except ProcessLookupError:
        click.echo("Process not found, cleaning up PID file")
        PID_FILE.unlink(missing_ok=True)
    except Exception as e:
        click.echo(f"Failed to stop Whisper Assistant: {e}", err=True)
        sys.exit(1)


@cli.command()
def status():
    """Check the status of Whisper Assistant."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is running (PID: {pid})")
    else:
        click.echo("Whisper Assistant is not running")


@cli.group()
def history():
    """Manage recorded history."""
    pass


@history.command()
def list():
    """List recorded history."""
    if not HISTORY_DIR.exists():
        click.echo("No history found")
        return

    # List all date directories
    date_dirs = sorted([d for d in HISTORY_DIR.iterdir() if d.is_dir()], reverse=True)

    if not date_dirs:
        click.echo("No recordings found")
        return

    for date_dir in date_dirs:
        click.echo("")
        # List timestamp directories in this date directory
        timestamp_dirs = sorted(
            [d for d in date_dir.iterdir() if d.is_dir()], reverse=True
        )
        for timestamp_dir in timestamp_dirs:
            transcription_file = timestamp_dir / "transcription.txt"
            has_transcription = transcription_file.exists()
            status = "✓" if has_transcription else "✗"
            # Output in YYYY-MM-DD-HHMMSS format for easy copy-paste
            datetime_str = f"{date_dir.name}-{timestamp_dir.name}"
            click.echo(datetime_str)


@history.command()
@click.argument("datetime")
def transcribe(datetime):
    """Transcribe a specific recording by datetime (YYYY-MM-DD-HHMMSS)."""
    # Parse datetime format: YYYY-MM-DD-HHMMSS
    # Split on last hyphen to separate date from time
    parts = datetime.rsplit("-", 1)
    if len(parts) != 2:
        click.echo(
            f"Invalid datetime format. Expected YYYY-MM-DD-HHMMSS, got: {datetime}",
            err=True,
        )
        sys.exit(1)

    date, time = parts
    # Construct path: history/YYYY-MM-DD/HHMMSS/recording.wav
    audio_path = HISTORY_DIR / date / time / "recording.wav"

    if not audio_path.exists():
        click.echo(f"Recording not found: {audio_path}", err=True)
        sys.exit(1)

    click.echo(f"Transcribing {audio_path}...")

    try:
        # Initialize transcriber
        transcriber = Transcriber()

        # Transcribe the file
        text = transcriber.transcribe(
            str(audio_path), language=config.TRANSCRIPTION_LANGUAGE
        )

        if text:
            click.echo(f"\n{'=' * 60}")
            click.echo(f"Transcription:")
            click.echo(f"{text}")
            click.echo(f"{'=' * 60}\n")
        else:
            click.echo("Transcription returned empty result", err=True)
            sys.exit(1)

    except Exception as e:
        click.echo(f"Error during transcription: {e}", err=True)
        sys.exit(1)


if __name__ == "__main__":
    cli()

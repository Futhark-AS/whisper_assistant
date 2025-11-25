import os
import sys
import signal
import click
import subprocess
import time
from pathlib import Path
from env import read_env
from packages.transcriber import Transcriber
from dotenv import dotenv_values

# Constants
HISTORY_DIR = Path("history")
PID_FILE = Path("whisper_assistant.pid")

env = read_env()


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


def _start_daemon():
    """Internal function to start the daemon. Returns True on success, False on failure."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is already running (PID: {pid})")
        return False

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
        return True
    except Exception as e:
        click.echo(f"Failed to start Whisper Assistant: {e}", err=True)
        return False


def _stop_daemon():
    """Internal function to stop the daemon. Returns True on success, False on failure."""
    if not is_running():
        click.echo("Whisper Assistant is not running")
        return False

    pid = get_pid()
    if pid is None:
        click.echo("Could not read PID file")
        return False

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
        return True
    except ProcessLookupError:
        click.echo("Process not found, cleaning up PID file")
        PID_FILE.unlink(missing_ok=True)
        return True
    except Exception as e:
        click.echo(f"Failed to stop Whisper Assistant: {e}", err=True)
        return False


@cli.command()
def start():
    """Start the Whisper Assistant daemon."""
    if not _start_daemon():
        sys.exit(1)


@cli.command()
def stop():
    """Stop the Whisper Assistant daemon."""
    if not _stop_daemon():
        sys.exit(1)


@cli.command()
def restart():
    """Restart the Whisper Assistant daemon."""
    _stop_daemon()
    # Small delay to ensure clean shutdown
    time.sleep(0.5)
    if not _start_daemon():
        sys.exit(1)


@cli.command()
def status():
    """Check the status of Whisper Assistant."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is running (PID: {pid})")
    else:
        click.echo("Whisper Assistant is not running")


@cli.command()
@click.argument("audio_file", type=click.Path(exists=True, path_type=Path))
@click.option(
    "--language",
    default=None,
    help="Language code for transcription (e.g., 'en', 'es'). Defaults to config value.",
)
def transcribe(audio_file, language):
    """Transcribe an audio file. Can be any audio file on your system."""
    # Resolve to absolute path
    audio_path = Path(audio_file).resolve()

    if not audio_path.exists():
        click.echo(f"Audio file not found: {audio_path}", err=True)
        sys.exit(1)

    if not audio_path.is_file():
        click.echo(f"Path is not a file: {audio_path}", err=True)
        sys.exit(1)

    click.echo(f"Transcribing {audio_path}...")

    try:
        # Initialize transcriber
        transcriber = Transcriber()

        # Use provided language or fall back to config
        language = language if language is not None else env.TRANSCRIPTION_LANGUAGE

        # Transcribe the file
        text = transcriber.transcribe(str(audio_path), language=language)

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
        # List timestamp directories in this date directory
        timestamp_dirs = sorted(
            [d for d in date_dir.iterdir() if d.is_dir()], reverse=True
        )
        for timestamp_dir in timestamp_dirs:
            # Output full path to recording.wav file
            audio_path = timestamp_dir / "recording.wav"
            if audio_path.exists():
                # Use absolute path for easy copy-paste
                click.echo(str(audio_path.resolve()))


@cli.group()
def config():
    """Manage configuration."""
    pass


@config.command(name="show")
def config_show():
    """Show current configuration values."""
    # Find .env file in workspace root
    env_file = Path.cwd() / ".env"

    if not env_file.exists():
        click.echo(
            "No .env file found. Create one to configure the application.", err=True
        )
        sys.exit(1)

    import json

    click.echo(f"Current Configuration:\n{env}")


@config.command(name="edit")
def config_edit():
    """Edit configuration in your default editor. Restarts daemon if running."""
    # Find .env file in workspace root
    env_file = Path.cwd() / ".env"

    if not env_file.exists():
        click.echo("No .env file found. Creating template...", err=True)
        # Create a template .env file
        template = """# Groq API Key (required)
# Get your API key from https://console.groq.com/
GROQ_API_KEY=your_api_key

# Format for hotkeys: modifier1+modifier2+...+key
# Modifiers: cmd, ctrl, shift, alt
# Example: "cmd+ctrl+shift+alt+c"
# Default: "cmd+ctrl+shift+alt+2"
# Note: This hotkey toggles recording on/off
TOGGLE_RECORDING_HOTKEY=ctrl+shift+1
RETRY_TRANSCRIPTION_HOTKEY=ctrl+shift+2

# Transcription language
# Set to a language code (e.g., "en" for English, "nb" for Norwegian Bokmål) to force a language.
# Leave unset or empty to automatically detect language.
# Examples: "en", "nb", "es", "fr"
TRANSCRIPTION_LANGUAGE=
"""
        env_file.write_text(template)
        click.echo(f"Created .env file at: {env_file.resolve()}")

    # Get the file's modification time before editing
    mtime_before = env_file.stat().st_mtime

    # Check if daemon is running
    was_running = is_running()

    # Open in editor (respect EDITOR env var, fallback to sensible defaults)
    editor = os.environ.get("EDITOR", os.environ.get("VISUAL", "nano"))

    try:
        # Open the editor (may or may not block depending on the editor)
        subprocess.run([editor, str(env_file)], check=True)

        # Prompt user to confirm they're done editing
        # This handles editors that don't block (like 'code', 'subl')
        click.echo("\nPress Enter when you're done editing and have saved the file...")
        input()

        # Check if file was modified
        mtime_after = env_file.stat().st_mtime

        if mtime_after != mtime_before:
            click.echo("Configuration updated.")

            # If daemon was running, restart it to apply changes
            if was_running:
                click.echo("Restarting daemon to apply changes...")
                _stop_daemon()
                time.sleep(0.5)
                _start_daemon()
        else:
            click.echo("No changes made.")

    except subprocess.CalledProcessError:
        click.echo("Editor exited with error", err=True)
        sys.exit(1)
    except FileNotFoundError:
        click.echo(
            f"Editor '{editor}' not found. Set EDITOR environment variable.", err=True
        )
        sys.exit(1)


if __name__ == "__main__":
    cli()

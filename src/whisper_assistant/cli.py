import os
import shutil
import signal
import subprocess
import sys
import time

import click
from pathlib import Path

from whisper_assistant import __version__
from whisper_assistant.env import read_env, ConfigErrors
from whisper_assistant.paths import (
    get_config_file,
    get_history_dir,
    get_log_dir,
    get_pid_file,
)
from whisper_assistant.packages.transcriber import Transcriber

# Constants
HISTORY_DIR = get_history_dir()
PID_FILE = get_pid_file()
STDERR_LOG = get_log_dir() / "stderr.log"


def get_pid():
    """Get the PID from the PID file, or None if invalid/missing."""
    if not PID_FILE.exists():
        return None
    try:
        return int(PID_FILE.read_text().strip())
    except (ValueError, FileNotFoundError):
        return None


def is_running():
    """Check if the daemon is running."""
    pid = get_pid()
    if pid is None:
        return False

    try:
        os.kill(pid, 0)
        return True
    except (ProcessLookupError, OSError):
        PID_FILE.unlink(missing_ok=True)
        return False


@click.group()
@click.version_option(
    version=__version__,
    prog_name="whisper-assistant",
    message="%(prog)s %(version)s\n\nTo upgrade: uv tool install whisper-assistant --force --from git+https://github.com/Futhark-AS/whisper_assistant.git",
)
def cli():
    """Whisper Assistant CLI."""
    pass


GROQ_CONSOLE_URL = "https://console.groq.com/keys"


@cli.command()
def init():
    """Interactive init wizard for first-time configuration."""
    config_file = get_config_file()

    click.echo()
    click.secho("🎙️  Whisper Assistant Setup", fg="cyan", bold=True)
    click.echo("=" * 40)
    click.echo()

    # Step 1: Groq API Key
    click.echo("This tool uses Groq's Whisper API for transcription.")
    click.echo()
    api_key = click.prompt(
        click.style(
            f"1. Get your free API key at: {GROQ_CONSOLE_URL} and paste it here",
            fg="yellow",
        )
    )

    if not api_key or api_key == "your_api_key":
        click.secho("Invalid API key. Run 'whisper-assistant init' again.", fg="red")
        sys.exit(1)

    click.echo()
    click.secho("3. Configure hotkeys and options", fg="yellow")
    click.echo("   (Press Enter to accept defaults)")
    click.echo()

    # Hotkeys with defaults
    toggle_hotkey = click.prompt(
        "   Toggle recording hotkey",
        default="ctrl+shift+1",
        show_default=True,
    )

    retry_hotkey = click.prompt(
        "   Retry transcription hotkey",
        default="ctrl+shift+2",
        show_default=True,
    )

    cancel_hotkey = click.prompt(
        "   Cancel recording hotkey",
        default="ctrl+shift+3",
        show_default=True,
    )

    # Language
    click.echo()
    click.echo("   Language: 'auto' for auto-detect, or code like 'en', 'no', 'es'")
    language = click.prompt(
        "   Transcription language",
        default="auto",
        show_default=True,
    )

    # Output mode
    click.echo()
    click.echo(
        "   Output: 'clipboard', 'paste_on_cursor', 'clipboard,paste_on_cursor', or 'none'"
    )
    output = click.prompt(
        "   Transcription output",
        default="paste_on_cursor",
        show_default=True,
    )

    # Write config
    config_content = f"""\
# Whisper Assistant Configuration
# Edit with: whisper-assistant config edit

GROQ_API_KEY={api_key}

TOGGLE_RECORDING_HOTKEY={toggle_hotkey}

RETRY_TRANSCRIPTION_HOTKEY={retry_hotkey}

CANCEL_RECORDING_HOTKEY={cancel_hotkey}

TRANSCRIPTION_LANGUAGE={language}

TRANSCRIPTION_OUTPUT={output}
"""

    config_file.write_text(config_content)

    click.echo()
    click.secho("✅ Configuration saved!", fg="green", bold=True)
    click.echo(f"   Config file: {config_file}")
    click.echo()

    # Validate config
    try:
        read_env()
    except ConfigErrors as e:
        click.secho(f"⚠️  Config validation warning:\n{e}", fg="yellow")
        click.echo("   Run 'whisper-assistant config edit' to fix.")
        return

    # Offer to start daemon
    if click.confirm("Start whisper-assistant now?", default=True):
        click.echo()
        _start_daemon()
        click.echo()
        click.secho("🎉 You're all set!", fg="cyan", bold=True)
        click.echo(f"   Press {toggle_hotkey} to start recording")
        click.echo(f"   Press {toggle_hotkey} again to stop and transcribe")
    else:
        click.echo()
        click.echo("Run 'whisper-assistant start' when ready.")


def _start_daemon():
    """Internal function to start the daemon. Returns True on success, False on failure."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is already running (PID: {pid})")
        return False

    # Start daemon process
    try:
        # Use sys.executable to ensure we use the same Python interpreter
        # Redirect stderr to a file so we can check for startup errors
        stderr_file = open(STDERR_LOG, "w")
        process = subprocess.Popen(
            [sys.executable, "-m", "whisper_assistant.main"],
            stdout=subprocess.DEVNULL,
            stderr=stderr_file,
            start_new_session=True,  # Detach from parent process
        )

        # Write PID file
        PID_FILE.write_text(str(process.pid))

        # Wait briefly to detect immediate crashes
        time.sleep(0.3)

        # Check if process crashed during startup
        exit_code = process.poll()
        if exit_code is not None:
            stderr_file.close()
            click.echo(
                f"Whisper Assistant crashed during startup (exit code: {exit_code})",
                err=True,
            )
            # Show stderr output
            if STDERR_LOG.exists():
                stderr_content = STDERR_LOG.read_text().strip()
                if stderr_content:
                    click.echo(f"\nError output:\n{stderr_content}", err=True)
            PID_FILE.unlink(missing_ok=True)
            return False

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
@click.option("--lines", "-n", default=50, help="Number of lines to show (default: 50)")
@click.option("--stderr", is_flag=True, help="Show stderr log instead of main log")
def logs(lines, stderr):
    """Show recent logs from the daemon."""
    if stderr:
        log_file = STDERR_LOG
    else:
        log_file = get_log_dir() / "info.log"

    if not log_file.exists():
        click.echo(f"Log file not found: {log_file}")
        return

    content = log_file.read_text()
    log_lines = content.strip().split("\n")

    # Show last N lines
    recent = log_lines[-lines:] if len(log_lines) > lines else log_lines
    for line in recent:
        click.echo(line)


def _transcribe_audio(audio_path: Path, language: str | None):
    """Transcribe audio file and print result. Exits on error."""
    env = read_env()
    click.echo(f"Transcribing {audio_path}...")

    try:
        transcriber = Transcriber()
        lang = language if language is not None else env.TRANSCRIPTION_LANGUAGE
        text = transcriber.transcribe(str(audio_path), language=lang)

        if text:
            click.echo(f"\n{'=' * 60}")
            click.echo("Transcription:")
            click.echo(text)
            click.echo(f"{'=' * 60}\n")
        else:
            click.echo("Transcription returned empty result", err=True)
            sys.exit(1)

    except Exception as e:
        click.echo(f"Error during transcription: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("file", type=click.Path(exists=True, path_type=Path))
@click.option(
    "--language",
    default=None,
    help="Language code for transcription (e.g., 'en', 'es', 'no'). Defaults to config value.",
)
def transcribe(file, language):
    """Transcribe an audio or video file."""
    file_path = Path(file).resolve()

    if not file_path.exists():
        click.echo(f"File not found: {file_path}", err=True)
        sys.exit(1)

    if not file_path.is_file():
        click.echo(f"Path is not a file: {file_path}", err=True)
        sys.exit(1)

    _transcribe_audio(file_path, language)


@cli.group()
def history():
    """Manage recorded history."""
    pass


def _get_all_recordings():
    """Get all recordings sorted chronologically (oldest first)."""
    if not HISTORY_DIR.exists():
        return []

    recordings = []
    for date_dir in sorted(d for d in HISTORY_DIR.iterdir() if d.is_dir()):
        for timestamp_dir in sorted(d for d in date_dir.iterdir() if d.is_dir()):
            audio_path = timestamp_dir / "recording.wav"
            if audio_path.exists():
                recordings.append(audio_path)
    return recordings


@history.command()
def list():
    """List recorded history."""
    recordings = _get_all_recordings()

    if not recordings:
        click.echo("No recordings found")
        return

    for audio_path in recordings:
        click.echo(str(audio_path.resolve()))


@history.command(name="transcribe")
@click.argument("n", type=int)
@click.option(
    "--language",
    default=None,
    help="Language code for transcription (e.g., 'en', 'es', 'no'). Defaults to config value.",
)
def history_transcribe(n, language):
    """Transcribe the Nth most recent recording (1 = latest)."""
    if n < 1:
        click.echo("N must be at least 1", err=True)
        sys.exit(1)

    recordings = _get_all_recordings()

    if not recordings:
        click.echo("No recordings found", err=True)
        sys.exit(1)

    if n > len(recordings):
        click.echo(f"Only {len(recordings)} recordings available", err=True)
        sys.exit(1)

    _transcribe_audio(recordings[-n], language)


@history.command(name="play")
@click.argument("n", type=int)
def history_play(n):
    """Play the Nth most recent recording (1 = latest)."""
    if not shutil.which("afplay"):
        click.echo(
            "afplay not found. This command requires macOS with afplay installed.",
            err=True,
        )
        sys.exit(1)

    if n < 1:
        click.echo("N must be at least 1", err=True)
        sys.exit(1)

    recordings = _get_all_recordings()

    if not recordings:
        click.echo("No recordings found", err=True)
        sys.exit(1)

    if n > len(recordings):
        click.echo(f"Only {len(recordings)} recordings available", err=True)
        sys.exit(1)

    audio_path = recordings[-n]
    click.echo(f"Playing {audio_path}...")
    subprocess.run(["afplay", str(audio_path)])


@cli.group()
def config():
    """Manage configuration."""
    pass


@config.command(name="show")
def config_show():
    """Show current configuration values."""
    env = read_env()
    click.echo(f"Current Configuration:\n{env}")


@config.command(name="edit")
@click.option(
    "--editor",
    "-e",
    default=None,
    help="Editor to use (e.g., 'nvim', 'vim', 'code'). Defaults to $EDITOR or $VISUAL.",
)
def config_edit(editor):
    """Edit configuration in your default editor. Restarts daemon if running."""
    env_file = get_config_file()

    # Check if daemon is running
    was_running = is_running()

    # Use provided editor, or fall back to env vars
    if editor is None:
        editor = os.environ.get("EDITOR", os.environ.get("VISUAL", "code"))

    while True:
        try:
            # Open the editor (may or may not block depending on the editor)
            subprocess.run([editor, str(env_file)], check=True)

            # Prompt user to confirm they're done editing
            # This handles editors that don't block (like 'code', 'subl')
            click.echo(
                "\nPress Enter when you're done editing and have saved the file..."
            )
            input()

        except subprocess.CalledProcessError:
            click.echo("Editor exited with error", err=True)
            sys.exit(1)
        except FileNotFoundError:
            click.echo(
                f"Editor '{editor}' not found. Set EDITOR environment variable.",
                err=True,
            )
            sys.exit(1)

        # Validate configuration
        try:
            read_env()
            click.echo("Configuration valid.")
            break
        except ConfigErrors as e:
            click.echo(f"\n{e}", err=True)
            if not click.confirm("Edit again?", default=True):
                click.echo("Aborted.")
                return

    # If daemon was running, restart it to apply changes
    if was_running:
        click.echo("Restarting daemon to apply changes...")
        _stop_daemon()
        time.sleep(0.5)
        _start_daemon()


if __name__ == "__main__":
    cli()

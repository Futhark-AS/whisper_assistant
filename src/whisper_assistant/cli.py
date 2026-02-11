import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

import click

from whisper_assistant import __version__
from whisper_assistant.env import VALID_WHISPER_MODELS, ConfigErrors, read_env
from whisper_assistant.packages.transcriber import Transcriber
from whisper_assistant.paths import (
    get_config_file,
    get_history_dir,
    get_log_dir,
    get_pid_file,
)
from whisper_assistant.permissions import (
    SYM_FAIL,
    SYM_OK,
    SYM_WARN,
    check_all,
    check_microphone,
    open_settings,
)

# Constants
HISTORY_DIR = get_history_dir()
PID_FILE = get_pid_file()
STDERR_LOG = get_log_dir() / "stderr.log"

_PERM_LABELS: dict[str, str] = {
    "accessibility": "Accessibility",
    "input_monitoring": "Input Monitoring",
    "microphone": "Microphone",
}

_PERM_WHY: dict[str, str] = {
    "accessibility": "paste transcriptions at cursor",
    "input_monitoring": "global hotkeys",
    "microphone": "audio recording",
}

_PERM_DAEMON_REASONS: dict[str, str] = {
    "accessibility": "paste won't work",
    "input_monitoring": "hotkeys won't work",
    "microphone": "recording will be silent",
}

_PERM_FIX: dict[str, str] = {
    "accessibility": "System Settings > Privacy & Security > Accessibility — add your terminal app",
    "input_monitoring": "System Settings > Privacy & Security > Input Monitoring — add your terminal app",
    "microphone": "System Settings > Privacy & Security > Microphone — add your terminal app",
}


def _perm_status_line(name: str, granted: bool) -> None:
    """Print a single permission status line with color."""
    label = _PERM_LABELS[name]
    why = _PERM_WHY[name]
    if granted:
        click.secho(f"  {SYM_OK} {label:<20}{why}", fg="green")
    else:
        click.secho(f"  {SYM_FAIL} {label:<20}Not granted — needed for {why}", fg="red", bold=True)


def get_pid() -> int | None:
    """Get the PID from the PID file, or None if invalid/missing."""
    if not PID_FILE.exists():
        return None
    try:
        return int(PID_FILE.read_text().strip())
    except (ValueError, FileNotFoundError):
        return None


def is_running() -> bool:
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
def cli() -> None:
    """Whisper Assistant CLI."""
    pass


GROQ_CONSOLE_URL = "https://console.groq.com/keys"


@cli.command()
def init() -> None:
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

    # Step 2: Hotkeys
    click.echo()
    click.secho("2. Configure hotkeys", fg="yellow")
    click.echo("   (Press Enter to accept defaults)")
    click.echo()

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

    # Step 3: Language and output
    click.echo()
    click.secho("3. Language and output", fg="yellow")
    click.echo()

    click.echo("   Language: 'auto' for auto-detect, or code like 'en', 'no', 'es'")
    language = click.prompt(
        "   Transcription language",
        default="auto",
        show_default=True,
    )

    click.echo()
    click.echo(
        "   Output: 'clipboard', 'paste_on_cursor', 'clipboard,paste_on_cursor', or 'none'"
    )
    output = click.prompt(
        "   Transcription output",
        default="clipboard",
        show_default=True,
    )

    # Step 4: Model and timeout
    click.echo()
    click.secho("4. Model and timeout", fg="yellow")
    click.echo("   (Press Enter to accept defaults)")
    click.echo()

    valid_models = ", ".join(sorted(VALID_WHISPER_MODELS))
    click.echo(f"   Valid models: {valid_models}")
    whisper_model = click.prompt(
        "   Whisper model",
        default="whisper-large-v3",
        show_default=True,
    )

    groq_timeout = click.prompt(
        "   API timeout (seconds)",
        default=60,
        show_default=True,
        type=int,
    )

    # Step 5: Vocabulary hints
    click.echo()
    click.secho("5. Vocabulary hints", fg="yellow")
    click.echo("   Comma-separated words the model often mishears (e.g. Claude,Cloudgeni)")
    click.echo("   Leave blank to skip.")
    click.echo()

    vocabulary = click.prompt(
        "   Vocabulary",
        default="",
        show_default=False,
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

WHISPER_MODEL={whisper_model}

GROQ_TIMEOUT={groq_timeout}

VOCABULARY={vocabulary}
"""

    config_file.write_text(config_content)
    config_file.chmod(0o600)

    click.echo()
    click.secho("Configuration saved!", fg="green", bold=True)
    click.echo(f"   Config file: {config_file}")
    click.echo()

    # Validate config
    try:
        read_env()
    except ConfigErrors as e:
        click.secho(f"{SYM_WARN}  Config validation warning:\n{e}", fg="yellow")
        click.echo("   Run 'whisper-assistant config edit' to fix.")
        return

    # Permission check phase
    click.secho("Permissions", bold=True)
    click.echo("  (Grant these to your terminal app: Terminal, iTerm2, Ghostty, etc.)")
    click.echo()
    perms = check_all()
    for name in ("accessibility", "input_monitoring", "microphone"):
        _perm_status_line(name, perms[name])

    failed = [name for name, ok in perms.items() if not ok]
    if failed:
        click.echo()

        retries = 0
        while failed and retries < 2:
            if click.confirm("  Open System Settings to fix?", default=True):
                for name in failed:
                    open_settings(name)
                click.echo()
                click.echo("  Press Enter after granting permissions...")
                input()
                click.echo()
                click.secho("  Re-checking...", fg="yellow")
                click.echo()
                perms = check_all()
                for name in ("accessibility", "input_monitoring", "microphone"):
                    _perm_status_line(name, perms[name])
                failed = [name for name, ok in perms.items() if not ok]
                retries += 1
            else:
                break

        if failed:
            click.echo()
            click.echo("  You can fix later with: whisper-assistant doctor --fix")

    click.echo()

    # Offer to start daemon
    if click.confirm("Start whisper-assistant now?", default=True):
        click.echo()
        _start_daemon()
        click.echo()
        click.secho("You're all set!", fg="cyan", bold=True)
    else:
        click.echo()
        click.echo("Run 'whisper-assistant start' when ready.")


def _print_hotkey_info() -> None:
    """Print hotkey bindings and config location after daemon start."""
    try:
        from dotenv import dotenv_values

        config = dotenv_values(get_config_file())
        toggle = config.get("TOGGLE_RECORDING_HOTKEY", "ctrl+shift+1")
        retry = config.get("RETRY_TRANSCRIPTION_HOTKEY", "ctrl+shift+2")
        cancel = config.get("CANCEL_RECORDING_HOTKEY", "ctrl+shift+3")
        click.echo()
        click.secho("  Hotkeys:", bold=True)
        click.echo(f"    Toggle recording:     {toggle}")
        click.echo(f"    Retry transcription:  {retry}")
        click.echo(f"    Cancel recording:     {cancel}")
        click.echo()
        click.echo("  Change hotkeys: whisper-assistant config edit")
    except Exception:
        pass


def _start_daemon() -> bool:
    """Internal function to start the daemon. Returns True on success, False on failure."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is already running (PID: {pid})")
        return False

    # Quick permission check
    perms = check_all()
    failed = [name for name, ok in perms.items() if not ok]
    if failed:
        click.secho(f"{SYM_WARN}  Missing permissions:", fg="yellow")
        for name in failed:
            click.secho(
                f"   {_PERM_LABELS[name]:<20}{_PERM_DAEMON_REASONS[name]}",
                fg="yellow",
            )
        click.echo("   Fix with: whisper-assistant doctor --fix")
        click.echo()

    # Start daemon process
    try:
        # Use sys.executable to ensure we use the same Python interpreter
        # Redirect stderr to a file so we can check for startup errors
        with open(STDERR_LOG, "w") as stderr_file:
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
        _print_hotkey_info()
        return True
    except Exception as e:
        click.echo(f"Failed to start Whisper Assistant: {e}", err=True)
        return False


def _stop_daemon() -> bool:
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
def start() -> None:
    """Start the Whisper Assistant daemon."""
    if not _start_daemon():
        sys.exit(1)


@cli.command()
def stop() -> None:
    """Stop the Whisper Assistant daemon."""
    if not _stop_daemon():
        sys.exit(1)


@cli.command()
def restart() -> None:
    """Restart the Whisper Assistant daemon."""
    _stop_daemon()
    # Small delay to ensure clean shutdown
    time.sleep(0.5)
    if not _start_daemon():
        sys.exit(1)


@cli.command()
def status() -> None:
    """Check the status of Whisper Assistant."""
    if is_running():
        pid = get_pid()
        click.echo(f"Whisper Assistant is running (PID: {pid})")
    else:
        click.echo("Whisper Assistant is not running")


@cli.command()
@click.option("--fix", is_flag=True, help="Automatically open System Settings for missing permissions")
def doctor(fix: bool) -> None:
    """Diagnose common issues with permissions, config, daemon, and audio."""
    issues: list[str] = []

    # ── Permissions ──────────────────────────────────────────────────
    click.echo()
    click.secho("Permissions", bold=True)
    click.echo("  (Grant these to your terminal app: Terminal, iTerm2, Ghostty, etc.)")
    click.echo()
    perms = check_all()
    for name in ("accessibility", "input_monitoring", "microphone"):
        _perm_status_line(name, perms[name])
        if not perms[name]:
            issues.append(_PERM_FIX[name])
            if fix:
                open_settings(name)

    # ── Configuration ────────────────────────────────────────────────
    click.echo()
    click.secho("Configuration", bold=True)
    config_file = get_config_file()
    try:
        env = read_env()
        click.secho(f"  {SYM_OK} Config valid", fg="green")
        click.echo(f"     API key:  {env._masked_key()}")
        click.echo(f"     Model:    {env.WHISPER_MODEL}")
        click.echo(f"     File:     {config_file}")
    except ConfigErrors as e:
        click.secho(f"  {SYM_FAIL} Config invalid", fg="red", bold=True)
        for err in e.errors:
            click.echo(f"     {err}")
        click.echo(f"     File: {config_file}")
        issues.append("Fix config: whisper-assistant config edit")

    # ── Daemon ───────────────────────────────────────────────────────
    click.echo()
    click.secho("Daemon", bold=True)
    if is_running():
        pid = get_pid()
        click.secho(f"  {SYM_OK} Running (PID {pid})", fg="green")
    else:
        click.secho(f"  {SYM_WARN} Not running", fg="yellow")
        issues.append("Start daemon: whisper-assistant start")

    # ── Audio ────────────────────────────────────────────────────────
    click.echo()
    click.secho("Audio", bold=True)
    try:
        import sounddevice as sd

        default_dev = sd.query_devices(kind="input")
        dev_name = default_dev.get("name", "unknown") if isinstance(default_dev, dict) else "unknown"
        click.secho(f"  {SYM_OK} Input device: {dev_name}", fg="green")
    except Exception as e:
        click.secho(f"  {SYM_FAIL} Could not query audio devices: {e}", fg="red")
        issues.append("Check audio input device configuration")

    mic_ok, mic_msg = check_microphone()
    if mic_ok:
        click.secho(f"  {SYM_OK} Mic probe: {mic_msg}", fg="green")
    else:
        click.secho(f"  {SYM_WARN} Mic probe: {mic_msg}", fg="yellow")

    # ── Summary ──────────────────────────────────────────────────────
    click.echo()
    if not issues:
        click.secho(f"Everything looks good! {SYM_OK}", fg="green", bold=True)
    else:
        click.secho(f"Found {len(issues)} issue(s):", bold=True)
        for i, issue in enumerate(issues, 1):
            click.echo(f"  {i}. {issue}")

    if fix and any(not perms[n] for n in perms):
        click.echo()
        click.secho("  Opened System Settings for missing permissions.", fg="yellow")
        click.echo("  Add your terminal app, then re-run: whisper-assistant doctor")

    click.echo()


@cli.command()
@click.option("--lines", "-n", default=50, help="Number of lines to show (default: 50)")
@click.option("--stderr", is_flag=True, help="Show stderr log instead of main log")
def logs(lines: int, stderr: bool) -> None:
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


def _transcribe_audio(audio_path: Path, language: str | None) -> None:
    """Transcribe audio file and print result. Exits on error."""
    env = read_env()
    click.echo(f"Transcribing {audio_path}...")

    try:
        transcriber = Transcriber(model=env.WHISPER_MODEL, timeout=600)
        lang = language if language is not None else env.TRANSCRIPTION_LANGUAGE
        vocab_prompt = ", ".join(env.VOCABULARY) if env.VOCABULARY else ""
        text = transcriber.transcribe(str(audio_path), prompt=vocab_prompt, language=lang)

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
def transcribe(file: Path, language: str | None) -> None:
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
def history() -> None:
    """Manage recorded history."""
    pass


def _get_all_recordings() -> list[Path]:
    """Get all recordings sorted chronologically (oldest first)."""
    if not HISTORY_DIR.exists():
        return []

    recordings = []
    for date_dir in sorted(d for d in HISTORY_DIR.iterdir() if d.is_dir()):
        for timestamp_dir in sorted(d for d in date_dir.iterdir() if d.is_dir()):
            # Prefer .flac (new format) but fall back to .wav (old recordings)
            audio_path = timestamp_dir / "recording.flac"
            if not audio_path.exists():
                audio_path = timestamp_dir / "recording.wav"
            if audio_path.exists():
                recordings.append(audio_path)
    return recordings


@history.command(name="list")
def list_recordings() -> None:
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
def history_transcribe(n: int, language: str | None) -> None:
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
def history_play(n: int) -> None:
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
def config() -> None:
    """Manage configuration."""
    pass


@config.command(name="show")
def config_show() -> None:
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
def config_edit(editor: str | None) -> None:
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

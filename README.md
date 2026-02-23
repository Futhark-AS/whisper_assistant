# Whisper Assistant

Voice-to-text tool powered by Groq's Whisper API. Press a hotkey, speak, and get instant transcription.

## Get Started

Requires macOS and [uv](https://github.com/astral-sh/uv).

```bash
uv tool install whisper-assistant --from git+https://github.com/Futhark-AS/whisper_assistant.git
whisper-assistant --version  # shows how to upgrade
whisper-assistant init
```

The `init` wizard will:
1. Ask for your [Groq API key](https://console.groq.com/keys) (free)
2. Configure hotkeys and preferences
3. Start the background daemon

> **Note:** macOS Accessibility permissions are required for global hotkeys. Grant access in System Settings → Privacy & Security → Accessibility when prompted.

**Where files are stored** ([XDG Base Directory Specification](https://xdgbasedirectoryspecification.com/)):
| Path | Contents |
|------|----------|
| `~/.config/whisper-assistant/config.env` | API key, hotkeys, preferences |
| `~/.local/share/whisper-assistant/history/` | Audio recordings & transcriptions |
| `~/.local/state/whisper-assistant/logs/` | Log files |
| `~/.local/state/whisper-assistant/daemon.pid` | Daemon process ID |

## Usage

```bash
# Daemon control
whisper-assistant start
whisper-assistant stop
whisper-assistant restart
whisper-assistant status

# View logs
whisper-assistant logs
whisper-assistant logs --stderr

# Configuration
whisper-assistant config show
whisper-assistant config edit

# History
whisper-assistant history list
whisper-assistant history play 1      # play most recent recording
whisper-assistant history transcribe 1  # re-transcribe most recent

# Transcribe any audio file
whisper-assistant transcribe /path/to/audio.wav
```

## macOS App Downloads (GitHub Releases)

For friends who want a direct app download:

1. Open GitHub Releases and download `WhisperAssistant.dmg` (or `WhisperAssistant.app.zip`).
2. Move `WhisperAssistant.app` to `/Applications`.
3. Launch app and grant requested permissions.

Release publishing is automated:

- Push a tag like `v1.2.3`
- GitHub Actions builds and uploads:
  - `WhisperAssistant.dmg`
  - `WhisperAssistant.app.zip`
  - `wa-macos.zip`
  - `SHA256SUMS.txt`

Optional signing/notarization in CI is enabled when these repo secrets exist:
- `APPLE_CERTIFICATE_P12` (base64-encoded Developer ID Application `.p12`)
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY` (optional; auto-resolved if omitted)
- `APPLE_KEYCHAIN_PASSWORD` (optional)
- `APPLE_ID`
- `APPLE_TEAM_ID`
- `APPLE_APP_PASSWORD`

**Default hotkeys:**
- `Ctrl+Shift+1` — Toggle recording (press to start, press again to stop and transcribe)
- `Ctrl+Shift+2` — Retry last transcription
- `Ctrl+Shift+3` — Cancel recording

**Config location:** `~/.config/whisper-assistant/config.env`

## About uv tools

This CLI is installed via `uv tool install`, which:
- Creates an isolated venv at `~/.local/share/uv/tools/whisper-assistant/`
- Symlinks the CLI to `~/.local/bin/` (should be in your PATH)

If you want to use a shorter name for running the CLI, you can symlink it:

```bash
ln -sf ~/.local/bin/whisper-assistant ~/.local/bin/whisp
```

**Why git instead of PyPI?**
- Faster iteration without publishing releases
- Always get the latest from `main`

**Trade-off:** No pinned versions — you always pull latest. For stability, pin to a commit:
```bash
uv tool install whisper-assistant --from git+https://github.com/Futhark-AS/whisper_assistant.git@<commit-sha>
```

**Useful commands:**
```bash
uv tool list       # see installed tools
uv tool uninstall whisper-assistant
```

## Contributing

PRs welcome. For major changes, open an issue first.

```bash
git clone https://github.com/Futhark-AS/whisper_assistant.git
cd whisper_assistant
uv sync
uv run whisper-assistant --help
```

[MIT License](LICENSE)

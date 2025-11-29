# Whisper Assistant :D 

A lightweight, globally accessible voice-to-text tool powered by Groq's fast inference API.

Whisper Assistant sits in the background and listens for your custom hotkey. When triggered, it records your voice, transcribes it using the Groq API (Whisper model), and automatically copies the text to your clipboard.

## Features

- 🚀 **Blazing Fast Transcription**: Uses Groq's API for near-instant speech-to-text.
- ⌨️ **Global Hotkeys**: Toggle recording or retry transcription from anywhere in your OS.
- 📋 **Clipboard Integration**: Transcribed text is automatically copied to your clipboard.
- ⚙️ **Simple Configuration**: Easy setup via environment variables.
- 🖥️ **Flexible Usage**: Run interactively in the terminal or as a background daemon.

## Prerequisites

- Python 3.10+
- [uv](https://github.com/astral-sh/uv) package manager
- A [Groq API Key](https://console.groq.com/)

## Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/Futhark-AS/whisper_assistant.git
   cd whisper_assistant
   ```

2. **Install dependencies**
   ```bash
   uv sync
   ```

   *Note for macOS users: If you encounter issues installing PyAudio, you may need to install portaudio:*
   ```bash
   brew install portaudio
   ```

3. **Configure Environment**
   Create a `.env` file in the project root:
   ```bash
   cp .env.example .env
   ```

   Edit `.env` with your settings

## Usage

You can run the assistant in two modes: **Foreground** (Interactive) or **Daemon** (Background).

### Foreground Mode

Run the assistant directly in your terminal. This is useful for debugging or if you prefer to see logs in real-time. The process will stop when you close the terminal.

```bash
uv run src/main.py
```

### Daemon Mode (CLI)

Use the CLI to manage the assistant as a background process.

**Add as alias**
If using fish:
```bash
echo "alias whisp 'uv run --directory $PWD src/cli.py'" >> ~/.config/fish/config.fish
```

Then you can run the assistant like this anywhere:
```bash
whisp status
whisp start
whisp history list
whisp history transcribe YYYY-MM-DD-HHMMSS
whisp stop
```

**Start the daemon:**
```bash
uv run src/cli.py start
```

**Check status:**
```bash
uv run src/cli.py status
```

**Stop the daemon:**
```bash
uv run src/cli.py stop
```

### History Management

The CLI also provides tools to manage your recording history.

**List recordings:**
```bash
uv run src/cli.py history list
```

**Transcribe a specific recording:**
```bash
uv run src/cli.py history transcribe YYYY-MM-DD-HHMMSS
```

## Hotkeys

- **Toggle Recording**: Press your hotkey (configured in `.env`, e.g., `cmd+option+space`) to start recording. Press again to stop.
- **Retry Transcription**: Press your retry hotkey to re-transcribe the last recorded audio.

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)

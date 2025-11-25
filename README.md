# Whisper Assistant

A lightweight, globally accessible voice-to-text tool powered by Groq's fast inference API.

Whisper Assistant sits in the background and listens for your custom hotkey. When triggered, it records your voice, transcribes it using the Groq API (Whisper model), and automatically copies the text to your clipboard.

## Features

- 🚀 **Blazing Fast Transcription**: Uses Groq's API for near-instant speech-to-text.
- ⌨️ **Global Hotkeys**: Toggle recording or retry transcription from anywhere in your OS.
- 📋 **Clipboard Integration**: Transcribed text is automatically copied to your clipboard.
- ⚙️ **Simple Configuration**: Easy setup via environment variables.

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

Start the application:

```bash
uv run src/main.py
```

The application will run in the terminal. Use your configured hotkeys:
- **Toggle Recording**: Press your hotkey (e.g., `cmd+option+space`) to start recording. Press again to stop.
- **Retry Transcription**: Press your retry hotkey to re-transcribe the last recorded audio.

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)

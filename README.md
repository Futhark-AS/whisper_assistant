*Made by Jørgen Kristiansen Sandhaug and Henrik Skog*

# Whisper Assistant: A Voice-Powered Assistant and Transcriber


Whisper Assistant is a highly efficient and customizable voice-powered assistant designed to boost your productivity by seamlessly converting speech to text and performing a variety of tasks through voice commands. Built on the cutting-edge OpenAI Whisper API and the flexible LangChain framework, Whisper Assistant offers an easy-to-use interface for real-time voice transcription. It is also possible to integrate with large language models to execute complex tasks in a voice conversation fashion by creating custom agents in LangChain.
 

## Features

- **Highly Accurate Voice Transcription:** Utilizes the OpenAI Whisper API for the most accurate, real-time speech-to-text conversion for any language.
- **Customizable Shortcuts:** Easily set up your own keyboard shortcuts to activate the transcription feature or start talking to different assistants, making it effortlessly accessible anytime.
- **Versatile Voice Commands:** Execute tasks, manipulate text, or control your terminal directly with your voice using the LangChain framework.
- **Optimized for Efficiency:** Smartly stitches together audio segments to minimize costs, reduce transcription time, and improve accuracy, even in speech with gaps.
- **Visual Feedback:** A status icon in the menu bar shows the app's current state—whether it's recording, processing, or ready for your next command.
- **Designed for macOS:** Tailored for use on Apple Silicon Macs, ensuring smooth operation and compatibility.

## Getting Started

### Prerequisites

- Python 3.10 (or earlier possibly earlier) (cchardet does not seem to work well with Python 3.11 on Apple Silicon Macs).
- An OpenAI API key for using the Whisper API.

### Installation

1. **Clone the repository:**

   ```bash
   git clone https://github.com/Futhark-AS/whisper_assistant.git
   cd whisper_assistant
   ```

2. **Setup Conda Environment:**
   (This is the recommended way to set up the environment, as there has been many problems with the py2app build when not using conda)
   - Create a new conda environment:
     ```bash
     conda create -n whisper-assistant python=3.10
     conda activate whisper-assistant
     conda install pip
     ```

3. **Install Dependencies:**

   ```bash
   pip install -r requirements.txt
   ```

4. **Resolve Dependencies:**

   - For Apple Silicon Mac users encountering `libffi` issues:
     ```bash
     brew install libffi
     ```
     Follow the post-installation instructions from Homebrew to set up the necessary environment variables.

5. **Setup the environment:**

   - Create a `.env` file in the project root.
   - Add your OpenAI API key: `OPEN_AI_API_KEY=your_api_key_here`.


6. **Configure Your Shortcut:**

   - Copy the `shortcuts.py.template` file from the `config` folder.
   - Rename it to `shortcuts.py` and customize it with your preferred keyboard shortcut. Place this file in the `config` folder.

7. **Build the Application:**

   ```bash
   python setup.py py2app -A
   ```

8. **Allow your terminal to monitor input and microphone:**

   - If on Mac, go to `System Preferences` -> `Security & Privacy` -> `Privacy` -> `Accessibility`.
   - Add your terminal to the list of apps.

9. **Run Whisper Assistant:**

   ```bash
   ./dist/whisperGPT.app/Contents/MacOS/whisperGPT
   ```


## How to Use

- Press one of your configured shortcuts to start recording.
- Whisper Assistant will transcribe your speech and copy the text to your clipboard
- If the shortcut you pressed is for an action/assistant, the will run with the transcribed text as input.

## Contributing

We welcome contributions and suggestions! Feel free to fork the repository, make your changes, and submit a pull request. For major changes or questions, please open an issue first to discuss what you would like to change.

## License

Distributed under the MIT License. See `LICENSE` for more information.

## Acknowledgments

- OpenAI Whisper API for the speech-to-text engine.
- LangChain framework for enabling complex command executions.

---

This draft aims to cover the essential aspects of your project in a structured and reader-friendly manner. Feel free to customize the content to better fit your project's personality or add any additional sections you deem necessary, such as a 'Known Issues' or 'Future Work' section to outline ongoing development or potential enhancements.
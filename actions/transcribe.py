from actions.BaseAction import BaseAction
import pyperclip


class WhisperTranscribe(BaseAction):
    def __init__(self, shortcut):
        super().__init__(
            name="transcribe",
            action=lambda input_text: pyperclip.copy(input_text),
            description="Whisper Transcribe",
            shortcut=shortcut,
            config={
                "whisper_mode": "transcribe",
                "use_clipboard_input": False,
            }
        )
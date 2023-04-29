from BaseAction import BaseAction
import pyperclip
from shortcuts import super_key
from pynput import keyboard

hotkey_transcribe = super_key | {keyboard.KeyCode.from_char("2")}

class Transcribe(BaseAction):
    def __init__(self):
        super().__init__(
            name="transcribe",
            action=lambda input_text: pyperclip.copy(input_text),
            description="Whisper Transcribe",
            shortcut=hotkey_transcribe,
            config={
                "whisper_mode": "transcribe",
                "use_clipboard_input": False,
            }
        )
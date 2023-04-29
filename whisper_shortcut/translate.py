from BaseAction import BaseAction
import pyperclip
from shortcuts import super_key
from pynput import keyboard

hotkey_translate = super_key | {keyboard.KeyCode.from_char("<")}

class Translate(BaseAction):
    def __init__(self):
        super().__init__(
            name="translate",
            action=lambda input_text: pyperclip.copy(input_text),
            description="Whisper Translate",
            shortcut=hotkey_translate,
            config={
                "whisper_mode": "translate",
                "use_clipboard_input": False,
            }
        )
from actions.BaseAction import BaseAction
import pyperclip


class WhisperTranslate(BaseAction):
    def __init__(self, shortcut):
        super().__init__(
            name="translate",
            action=lambda input_text: pyperclip.copy(input_text),
            description="Whisper Translate",
            shortcut=shortcut,
            config={
                "whisper_mode": "translate",
                "use_clipboard_input": False,
                "whisper_prompt": "",
            }
        )
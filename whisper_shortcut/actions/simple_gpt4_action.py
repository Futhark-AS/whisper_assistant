from actions.BaseAction import BaseAction
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage
import pyperclip
from prompts import system_prompt_with_input
from shortcuts import super_key
from pynput import keyboard
from enum import Enum

from langchain.callbacks.streaming_stdout import StreamingStdOutCallbackHandler

class Models(Enum):
    GPT3 = "gpt-3.5-turbo",
    GPT4 = "gpt-4",

def simple_gpt_action(input_text, model_name = Models.GPT4):
    chat = ChatOpenAI(model_name=model_name.value)
    chat.request_timeout = 180

    response = chat(
        [
            SystemMessage(content=system_prompt_with_input),
            HumanMessage(content=input_text)
        ]
    )

    pyperclip.copy(response.content)

    return response.content

class SimpleGPTActionWithInput(BaseAction):
    model_name = Models.GPT3
    def __init__(self, shortcut, whisper_mode="translate", model_name: Models = Models.GPT3):
        self.model_name = model_name

        super().__init__(
            name="simple_gpt4_action",
            action=self.action,
            description="Simple GPT4 Action",
            shortcut=shortcut,
            config={
                "whisper_mode": whisper_mode,
            }
        )

    def action(self, input):
        clipboard = pyperclip.paste()

        prompt = f"""
        {clipboard}
        ==========
        {input}
        """

        return simple_gpt_action(prompt, model_name=self.model_name)

class SimpleGPT4Action(BaseAction):
    def __init__(self, shortcut):
        super().__init__(
            name="simple_gpt4_action",
            action=simple_gpt_action,
            description="Simple GPT4 Action",
            shortcut=shortcut,
            config={
                "whisper_mode": "transcribe",
                "use_clipboard_input": False,
            }
        )


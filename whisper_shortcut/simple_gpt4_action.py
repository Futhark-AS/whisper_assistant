from BaseAction import BaseAction
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage
import pyperclip
from prompts import system_prompt_with_input
from shortcuts import super_key
from pynput import keyboard

input_key = super_key | {keyboard.KeyCode.from_char("3")}
no_input_key = super_key | {keyboard.KeyCode.from_char("1")}

def simple_gpt4_action(input_text):
    chat = ChatOpenAI(model_name="gpt-4")
    chat.request_timeout = 180

    response = chat(
        [
            SystemMessage(content=system_prompt_with_input),
            HumanMessage(content=input_text)
        ]
    )

    pyperclip.copy(response.content)

    return response.content

class SimpleGPT4ActionWithInput(BaseAction):
    def __init__(self):
        super().__init__(
            name="simple_gpt4_action",
            action=simple_gpt4_action,
            description="Simple GPT4 Action",
            shortcut=input_key,
            config={
                "whisper_mode": "translate",
                "use_clipboard_input": True,
            }
        )

class SimpleGPT4Action(BaseAction):
    def __init__(self):
        super().__init__(
            name="simple_gpt4_action",
            action=simple_gpt4_action,
            description="Simple GPT4 Action",
            shortcut=no_input_key,
            config={
                "whisper_mode": "translate",
                "use_clipboard_input": False,
            }
        )
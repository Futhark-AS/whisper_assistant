from actions.BaseAction import BaseAction
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage
import pyperclip
from prompts import system_prompt_with_input
from shortcuts import super_key
from pynput import keyboard

gpt4_w_input = super_key | {keyboard.KeyCode.from_char("3")}
gpt4_no_input = super_key | {keyboard.KeyCode.from_char("1")}
gpt3_w_input = super_key | {keyboard.KeyCode.from_char("4")}
gpt3_no_input = super_key | {keyboard.KeyCode.from_char("5")}

class GPTTask(BaseAction):
    def __init__(self, model_name, use_clipboard_input, shortcut):
        self.model_name = model_name
        self.use_clipboard_input = use_clipboard_input
        super().__init__(
            name=f"single_gpt_{model_name} with input" if use_clipboard_input else f"single_gpt_{model_name} without input",
            description=f"single answer gpt {model_name}",
            action=self.chat_task,
            shortcut=shortcut,  
            config={
                "whisper_mode": "translate",
                "use_clipboard_input": use_clipboard_input,
            }
        )


    def chat_task(self, input_text):
        chat = ChatOpenAI(model_name=self.model_name)
        chat.request_timeout = 180

        response = chat(
            [
                SystemMessage(content=system_prompt_with_input),
                HumanMessage(content=input_text)
            ]
        )

        pyperclip.copy(response.content)

        return response.content

class BasicGPT4TaskWithInput(GPTTask):
    def __init__(self):
        super().__init__(
            model_name="gpt-4",
            use_clipboard_input=True,
            shortcut=gpt4_w_input
        )

class BasicGPT4Task(GPTTask):
    def __init__(self):
        super().__init__(
            model_name="gpt-4",
            use_clipboard_input=False,
            shortcut=gpt4_no_input
        )


class BasicGPT35TaskWithInput(GPTTask):
    def __init__(self):
        super().__init__(
            model_name="gpt-3.5-turbo",
            use_clipboard_input=True,
            shortcut=gpt3_w_input
        )

class BasicGPT35Task(GPTTask):
    def __init__(self):
        super().__init__(
            model_name="gpt-3.5-turbo",
            use_clipboard_input=False,
            shortcut=gpt3_no_input
        )
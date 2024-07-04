from actions.BaseAction import BaseAction
from langchain_openai import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage, AIMessage
import pyperclip
from prompts.prompts import system_prompt_with_input, system_prompt_default
from config.shortcuts import super_key
from pynput import keyboard
from enum import Enum
from config.config import Config

cfg = Config()

from langchain.callbacks.streaming_stdout import StreamingStdOutCallbackHandler
from langchain_groq import ChatGroq


import os

class Model(Enum):
    GPT3 = "gpt-3.5-turbo"
    GPT4 = "gpt-4"
    GPT4o = "gpt-4o"
    GROQ_LLAMA_3_8B = "llama3-8b-8192"
    GROQ_LLAMA_3_70B = "llama3-70b-8192"
    GROQ_MIXTRAL = "mistral-7b-32768"

last_chat = None

def simple_gpt_action(input_text, model_name = Model.GPT4, system_prompt=system_prompt_with_input, followup=False):
    global last_chat

    if model_name.value.startswith("llama"):
        chat = ChatGroq(model_name=model_name.value)
    elif model_name.value.startswith("mistral"):
        chat = ChatGroq(model_name=model_name.value)
    else:
        chat = ChatOpenAI(model_name=model_name.value)
    chat.request_timeout = 180

    new_msg = HumanMessage(content=input_text)

    if last_chat is not None and followup: 
        last_chat = last_chat + [new_msg]
    else:
        last_chat = [
                SystemMessage(content=system_prompt),
                HumanMessage(content=input_text)
            ] 

    response = chat(last_chat)

    last_chat = last_chat + [AIMessage(content=response.content)]

    pyperclip.copy(response.content)

    print(response.content)

    return response.content

class SimpleGPTActionWithInput(BaseAction):
    def __init__(self, shortcut, whisper_mode="transcribe", model_name: Model = Model.GPT3, followup=False):
        self.model_name = model_name
        self.followup = followup

        super().__init__(
            name=f"simple_{model_name.value}_action_with_input",
            action=self.action,
            description=f"Simple {model_name.value} Action with Input",
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

        return simple_gpt_action(prompt, model_name=self.model_name, followup=self.followup, system_prompt=system_prompt_with_input)

class SimpleGPTAction(BaseAction):
    def __init__(self, shortcut, model_name: Model = Model.GPT4, followup=False):
        super().__init__(
            name=f"simple_{model_name.value}_action_no_input",
            action=lambda inp: simple_gpt_action(inp, model_name=model_name, followup=followup, system_prompt=cfg.system_prompt),
            description=f"Simple {model_name.value} Action",
            shortcut=shortcut,
            config={
                "whisper_mode": "transcribe",
                "use_clipboard_input": False,
            }
        )


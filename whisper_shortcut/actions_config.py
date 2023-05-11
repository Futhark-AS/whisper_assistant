import logging
from shortcuts import super_key
from langchain.agents import Tool, AgentType
from langchain.memory import ConversationBufferMemory
from langchain.chat_models import ChatOpenAI
from langchain.utilities import SerpAPIWrapper
from langchain.agents import initialize_agent
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage, AIMessage
import pyperclip
from prompts import system_prompt_with_input, system_prompt_without_input, user_prompt_template
from shortcuts import super_key
from pynput import keyboard
from actions.simple_gpt_action import SimpleGPTAction, SimpleGPTActionWithInput, Models
from actions.translate import Translate
from actions.transcribe import Transcribe
from actions.agents.langchain_code_agent import LangchainCodeAgent, LangchainCodeAgentInput
from actions.agents.search_agent import GoogleSearchAgent, GoogleSearchReactAgent
from actions.agents.bash import BashAgentStartFolder
from actions.agents.executor import ExecutorAgent, ExecutorAgentWithInput
# from actions.agents.zapier import ZapierAgent, ZapierAgentInput
from actions.code_shortener import CodeShortener
from pynput import keyboard

from shortcuts import super_key

logger = logging.getLogger()

actions = [
    Transcribe(shortcut=super_key | {keyboard.KeyCode.from_char("2")}),
    SimpleGPTActionWithInput(shortcut=super_key | {keyboard.KeyCode.from_char("3")}, whisper_mode="transcribe", model_name=Models.GPT3),
    SimpleGPTActionWithInput(shortcut=super_key | {keyboard.KeyCode.from_char("4")}, whisper_mode="transcribe", model_name=Models.GPT4),
    SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("5")}, model_name=Models.GPT3),
    SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("6")}, model_name=Models.GPT4),
    GoogleSearchReactAgent(shortcut=super_key | {keyboard.KeyCode.from_char("8")}),
    SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("9")}, model_name=Models.GPT4, followup=True),
]



print("Ready")

import logging
from BaseAction import BaseAction
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
from simple_gpt4_action import SimpleGPT4Action, SimpleGPT4ActionWithInput
from translate import Translate
from transcribe import Transcribe
from langchain_code_agent import LangchainCodeAgent, LangchainCodeAgentInput
from search_agent import GoogleSearchAgent, GoogleSearchReactAgent

logger = logging.getLogger()

actions = [
    LangchainCodeAgent(),
    LangchainCodeAgentInput(),
    GoogleSearchAgent(),
    GoogleSearchReactAgent(),
    Translate(),
    Transcribe(),
    SimpleGPT4Action(),
    SimpleGPT4ActionWithInput(),
]
print("Ready")
for action in actions:
    print(action.name, action.shortcut)
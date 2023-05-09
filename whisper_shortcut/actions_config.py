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
from actions.single_gpt_action import BasicGPT4Task, BasicGPT4TaskWithInput, BasicGPT35Task, BasicGPT35TaskWithInput
from actions.translate import Translate
from actions.transcribe import Transcribe
from actions.agents.langchain_code_agent import LangchainCodeAgent, LangchainCodeAgentInput
from actions.agents.search_agent import GoogleSearchAgent, GoogleSearchReactAgent
from actions.agents.bash import BashAgentStartFolder
from actions.agents.executor import ExecutorAgent, ExecutorAgentWithInput
# from actions.agents.zapier import ZapierAgent, ZapierAgentInput

logger = logging.getLogger()

actions = [
    BasicGPT4Task(),
    BasicGPT4TaskWithInput(),
    BasicGPT35Task(),
    BasicGPT35TaskWithInput(),
    # LangchainCodeAgent(),
    # LangchainCodeAgentInput(),
    GoogleSearchAgent(),
    GoogleSearchReactAgent(),
    Translate(),
    Transcribe(),
    BashAgentStartFolder(),
    ExecutorAgent(),
    ExecutorAgentWithInput(),
    # ZapierAgent(),
    # ZapierAgentInput(),
]
print("Ready")
for action in actions:
    # shortcut except super_key
    print(f"super + {action.shortcut - super_key}", action.name)
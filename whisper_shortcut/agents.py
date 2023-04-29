import logging

from langchain.agents import Tool
from langchain.agents import AgentType
from langchain.memory import ConversationBufferMemory
from langchain.chat_models import ChatOpenAI
from langchain.utilities import SerpAPIWrapper
from langchain.agents import initialize_agent
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage, AIMessage
import pyperclip

from prompts import system_prompt_with_input, system_prompt_without_input, user_prompt_template


logger = logging.getLogger()

def simple_agent(input_text, system_prompt):
    chat = ChatOpenAI(model_name="gpt-4")

    response = chat(
        [
            SystemMessage(content=system_prompt),
            HumanMessage(content=input_text)
        ]
    )

    pyperclip.copy(response.content)

    return response.content


agents = {
    "simple_agent": simple_agent
}
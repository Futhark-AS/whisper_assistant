from langchain.utilities import GoogleSerperAPIWrapper
from langchain.chat_models import ChatOpenAI
from langchain.agents import initialize_agent, Tool, AgentType, load_tools
from actions.BaseAction import BaseAction
from config.shortcuts import super_key
from pynput import keyboard
import os

from langchain.utilities import SerpAPIWrapper 


llm = ChatOpenAI(temperature=0, model_name="gpt-4")
search = GoogleSerperAPIWrapper(serper_api_key=os.environ.get("SERPER_API_KEY"))

tools = load_tools(["serpapi"]) +     [Tool(
        name="Intermediate Answer",
        func=search.run,
        description="useful for when you need to ask with search"
    )]


self_ask_with_search = initialize_agent(tools, llm, agent=AgentType.CHAT_ZERO_SHOT_REACT_DESCRIPTION, verbose=True)

from actions.agents.utils.agent_template import custom_agent_executor
react_agent = custom_agent_executor(tools, llm)

class GoogleSearchAgent(BaseAction):
    def __init__(self, shortcut):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False,
        }

        super().__init__(
            name="google_search",
            description="agent that searches Google repeatedly to come up with an answer.",
            action=self_ask_with_search.run,
            shortcut=shortcut,
            config=config
        )


class GoogleSearchReactAgent(BaseAction):
    def __init__(self, shortcut):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False,
        }

        super().__init__(
            name="google_search_react",
            description="agent that searches Google repeatedly to come up with an answer.",
            action=self_ask_with_search.run,
            shortcut=shortcut,
            config=config
        )
from langchain.utilities import GoogleSerperAPIWrapper
from langchain.utilities import GoogleSerperAPIWrapper
from langchain.chat_models import ChatOpenAI
from langchain.agents import initialize_agent, Tool
from langchain.agents import AgentType
from BaseAction import BaseAction
from shortcuts import super_key
from pynput import keyboard

google_search_key = super_key | {keyboard.KeyCode.from_char("7")}
google_search_react_key = super_key | {keyboard.KeyCode.from_char("6")}

llm = ChatOpenAI(temperature=0, model_name="gpt-3.5-turbo")
search = GoogleSerperAPIWrapper()
tools = [
    Tool(
        name="Intermediate Answer",
        func=search.run,
        description="useful for when you need to ask with search"
    )
]
self_ask_with_search = initialize_agent(tools, llm, agent=AgentType.SELF_ASK_WITH_SEARCH, verbose=True)

react_agent = initialize_agent(tools, llm, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)


class GoogleSearchAgent(BaseAction):
    def __init__(self):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False,
        }

        super().__init__(
            name="google_search",
            description="agent that searches Google repeatedly to come up with an answer.",
            action=self_ask_with_search.run,
            shortcut=google_search_key,
            config=config
        )


class GoogleSearchReactAgent(BaseAction):
    def __init__(self):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False,
        }

        super().__init__(
            name="google_search_react",
            description="agent that searches Google repeatedly to come up with an answer.",
            action=react_agent.run,
            shortcut=google_search_react_key,
            config=config
        )
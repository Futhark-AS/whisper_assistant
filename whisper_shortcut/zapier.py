from langchain.chat_models import ChatOpenAI
from langchain.agents import initialize_agent, Tool, AgentType
from langchain.agents.agent_toolkits import ZapierToolkit
from langchain.utilities.zapier import ZapierNLAWrapper
from BaseAction import BaseAction
from shortcuts import super_key
from pynput import keyboard
from agent_template import custom_agent_executor

llm = ChatOpenAI(temperature=0, model_name="gpt-4")
zapier = ZapierNLAWrapper()
toolkit = ZapierToolkit.from_zapier_nla_wrapper(zapier)
# for tool in toolkit.get_tools():
#     print(tool)
#     print()
agent = initialize_agent(toolkit.get_tools(), llm, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)
# # # tools = list(map(lambda tool: tool + {"func": lambda input_text: tool._run(input_text)}, toolkit.get_tools()))
# # tools = []
# # for tool in toolkit.get_tools():
# #     new_tool = Tool(
# #         name=tool.name,
# #         description=tool.description,
# #         func=lambda input_text: tool._run(input_text),
# #     )
# #     tools.append(new_tool)
# # # print(tools)
# agent = custom_agent_executor(tools, llm)

# print(agent.tools)

class ZapierAgent(BaseAction):
    def __init__(self):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False,
        }

        super().__init__(
            name="zapier",
            description="agent that uses Zapier to automate tasks.",
            action=agent.run,
            shortcut=super_key | {keyboard.KeyCode.from_char("9")},
            config=config
        )

class ZapierAgentInput(BaseAction):
    def __init__(self):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": True,
        }

        super().__init__(
            name="zapier_with_clipboard",
            description="agent that uses Zapier to automate tasks.",
            action=agent.run,
            shortcut=super_key | {keyboard.KeyCode.from_char("0")},
            config=config
        )
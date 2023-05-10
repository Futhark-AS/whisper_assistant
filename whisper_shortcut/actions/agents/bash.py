from langchain.utilities import BashProcess
from langchain.chat_models import ChatOpenAI
from langchain.agents import initialize_agent, Tool, AgentType
from actions.BaseAction import BaseAction
from langchain.chains import LLMBashChain

from shortcuts import super_key
from pynput import keyboard

bash = BashProcess(persistent=True)
# chat = ChatOpenAI(temperature=0, model_name="gpt-4")
chat2 = ChatOpenAI(temperature=0, model_name="gpt-4")
# bashChain = LLMBashChain(llm=chat, bash_process=bash, verbose=True)
tools = [
    Tool(
        name="bash executor",
        func=lambda input_text: bash.run(input_text),
        description="Always use this tool for commands that you want to run in bash. Remember to use the correct format of Thought, Action, Action Input in your response when using this tool."
    )
]
# agent = initialize_agent(tools, chat2, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)

from actions.agents.utils.agent_template import custom_agent_executor
agent = custom_agent_executor(tools, chat2)

# print(agent.agent.output_parser.get_format_instructions())



class BashAgentStartFolder(BaseAction):
    def __init__(self, shortcut):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": True
        }

        super().__init__(
            name="bash",
            description="agent that runs bash commands",
            action=None,
            shortcut=shortcut,
            config=config
        )

        def action(input_text):
            #- Context from clipboard -\n%s\n- Context end -\n
            folder = input_text.split("\n")[1]
            bash.run("cd " + folder)
            agent.run(input_text.split("\n")[3].split("Instruction: ")[1])

        self.action = action







from langchain.utilities import BashProcess
from langchain.chat_models import ChatOpenAI
from langchain.agents import initialize_agent, Tool, AgentType
from actions.BaseAction import BaseAction
from langchain.utilities import PythonREPL
from langchain.chains import LLMBashChain

from actions.agents.utils.custom_repl import CustomREPL
from langchain.tools.human.tool import HumanInputRun


from shortcuts import super_key
from pynput import keyboard

bash = BashProcess(persistent=True)
python_repl = CustomREPL()
python_repl.bash_process = bash
chat = ChatOpenAI(temperature=0, model_name="gpt-4")
chat.request_timeout = 240
# bashChain = LLMBashChain(llm=chat, bash_process=bash, verbose=True)
tools = [
    Tool(
        name="bash executor",
        func=lambda input_text: bash.run(input_text),
        description="Always use this tool for commands that you want to run in bash, for example, if there is a pip library you need to install. To use pip, use the following bash: pip install x. Remember to use the correct format of Thought, Action, Action Input in your response when using this tool."
    ),
    Tool(
        name="python executor",
        func=python_repl.run,
        description="Always use this tool for writing and executing python code. Never give the code back to the user, try this tool until it works instead. When using this tool only provide the raw Python code in the Action Input. You have to provide the complete code to run when calling this tool. Remember to ALWAYS the correct format of Thought, Action, Action Input in your response when using this tool. If you get an empty observation, that means the code is running and the user is seeing it." 
    ),
    Tool(
        name="human input",
        func=HumanInputRun().run,
        description="Always use this tool for human input when you need acclaration on or specification of things that only the human can give you, or if you need help. Remember to use the correct format of Thought, Action, Action Input in your response when using this tool."
    )
]
# agent = initialize_agent(tools, chat2, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)

from actions.agents.utils.agent_template import custom_agent_executor
agent = custom_agent_executor(tools, chat)

# print(agent.agent.output_parser.get_format_instructions())



class ExecutorAgent(BaseAction):
    def __init__(self):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": False
        }

        super().__init__(
            name="bash",
            description="agent that runs python code or bash commands",
            action=None,
            shortcut=super_key | {keyboard.KeyCode.from_char("+")},
            config=config
        )

        def action(input_text):
            bash.run("source ~/.zshrc")
            bash.run("conda activate agents")
            agent.run(input_text)

        self.action = action







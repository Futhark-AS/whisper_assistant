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
chat = ChatOpenAI(temperature=0, model_name="gpt-3.5-turbo")
chat.request_timeout = 240
# bashChain = LLMBashChain(llm=chat, bash_process=bash, verbose=True)
tools = [
    Tool(
        name="bash_executor",
        func=lambda input_text: bash.run(input_text),
        description="Always use this tool for commands that you want to run in bash, for example, if there is a pip library you need to install. To use pip, use the following bash: pip install x. Remember to use the correct format of Thought, Action, Action Input in your response when using this tool."
    ),
    Tool(
        name="python_executor",
        func=python_repl.run,
        description="Always use this tool for writing and executing full python code. Always start your python code with lots of detailed python comments on how the code should work and the high-level structure. Use lots of comments inside other places of the code aswell. You have to provide the complete code every time you use this tool, NOT just parts of the code. Remember to ALWAYS the correct format of Thought, Action, Action Input in your response when using this tool. If you get an empty observation, that means the code is running and the user is seeing it." 
    ),
    # Tool(
    #     name="append python code to bottom and run",
    #     func=python_repl.add_code,
    #     description="Only use this tool to append code to the code you have already written in this session. DONT WRITE ALL THE CODE, JUST THE NEW CODE. If you need to replace previous code, use the python executor tool and NOT this. When using this tool only provide the raw Python code in the Action Input. Remember to ALWAYS the correct format of Thought, Action, Action Input in your response when using this tool. If you get an empty observation, that means the code is running and the user is seeing it."
    # ),
    Tool(
        name="human",
        func=HumanInputRun().run,
        description="Use this tool in the beginning to give the human your ideas for how to structure the code, including your reflections. If the human says 'build', start writing code! Remember to use the correct format of Thought, Action, Action Input in your response when using this tool."
    )
]
# agent = initialize_agent(tools, chat2, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)

from actions.agents.utils.agent_template import custom_agent_executor
agent = custom_agent_executor(tools, chat)

# print(agent.agent.output_parser.get_format_instructions())



class ExecutorAgent(BaseAction):
    def __init__(self, shortcut, whisper_mode="translate"): 
        config = {
            "whisper_mode": whisper_mode,
            "use_clipboard_input": False
        }

        super().__init__(
            name="python and bash executor no input",
            description="agent that runs python code or bash commands",
            action=None,
            shortcut=super_key | {keyboard.KeyCode.from_char("-")},
            config=config
        )

        def action(input_text):
            bash.run("source ~/.zshrc")
            bash.run("conda activate agents")
            bash.run("rm temp123.py")
            agent.run(input_text)

        self.action = action

class ExecutorAgentWithInput(BaseAction):
    def __init__(self, shortcut):
        config = {
            "whisper_mode": "translate",
            "use_clipboard_input": True
        }

        super().__init__(
            name="python and bash executor with input",
            description="agent that runs python code or bash commands",
            action=None,
            shortcut=shortcut,
            config=config
        )

        def action(input_text):
            bash.run("source .venv/bin/activate")
            agent.run(input_text)

        self.action = action






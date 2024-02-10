from langchain.agents import Tool, AgentExecutor, LLMSingleActionAgent, AgentOutputParser
from langchain.prompts import BaseChatPromptTemplate
from langchain import SerpAPIWrapper, LLMChain
from langchain.chat_models import ChatOpenAI
from typing import List, Union
from langchain.schema import AgentAction, AgentFinish, HumanMessage, SystemMessage
import logging
logger = logging.getLogger()
import re
# Set up the base template

from langchain.agents.mrkl.prompt import FORMAT_INSTRUCTIONS, PREFIX, SUFFIX

template = """Complete the Instruction given as best you can. You have access to the following tools:

{tools}

Use the following format in your response (you can only do one action at a time):

Thought: you should always think about what to do
Action: the action to take, should be one of [{tool_names}] (i.e. Action: tool_name)
Action Input: the input to the action

Then you will see the following:
Observation: the result of the action ... (this Thought/Action/Action Input/Observation can repeat N times, 1 for each chat response) 


When you are finished with your task, use the following format to stop:
Thought: ...
Final Answer: the final output of your task


Instruction: {input}
{agent_scratchpad}


ALWAYS follow STRICTLY one of the above formats in your response ("Thought" + "Action" + "Action Input" together || "Thought + "Final Answer" together) or the chat will not work.

Never answer with only thoughts.
"""

# Set up a prompt template
class CustomPromptTemplate(BaseChatPromptTemplate):
    # The template to use
    template: str
    # The list of tools available
    tools: List[Tool]
    
    def format_messages(self, **kwargs) -> str:
        # Get the intermediate steps (AgentAction, Observation tuples)
        # Format them in a particular way
        intermediate_steps = kwargs.pop("intermediate_steps")
        thoughts = ""
        for action, observation in intermediate_steps:
            thoughts += action.log
            thoughts += f"\nObservation: {observation}"
        # Set the agent_scratchpad variable to that value
        kwargs["agent_scratchpad"] = thoughts
        # Create a tools variable from the list of tools provided
        kwargs["tools"] = "\n".join([f"{tool.name}: {tool.description}" for tool in self.tools])
        # Create a list of tool names for the tools provided
        kwargs["tool_names"] = ", ".join([tool.name for tool in self.tools])
        formatted = self.template.format(**kwargs)
        return [SystemMessage(content=formatted)]
    

class CustomOutputParser(AgentOutputParser):
    def parse(self, llm_output: str) -> Union[AgentAction, AgentFinish]:
        # Check if agent should finish
        if "Final Answer:" in llm_output:
            return AgentFinish(
                # Return values is generally always a dictionary with a single `output` key
                # It is not recommended to try anything else at the moment :)
                return_values={"output": llm_output.split("Final Answer:")[-1].strip()},
                log=llm_output,
            )
        # Parse out the action and action input
        regex = r"Action\s*\d*\s*:(.*?)\nAction\s*\d*\s*Input\s*\d*\s*:[\s]*(.*)"
        match = re.search(regex, llm_output, re.DOTALL)
        if not match:
            logger.warning(f"Could not parse LLM output: `{llm_output}`")
            # raise ValueError(f"Could not parse LLM output: `{llm_output}`")
            AgentFinish(typename="AgentFinish", return_values={"output": llm_output}, log=llm_output)
        action = match.group(1).strip()
        action_input = match.group(2)

        # for SAM
        "/path/to/file.jpg | text | 0.9 | 0.8 | 0.85\n"
        "/path/to/file.jpg|text|0.9|0.8|0.85"
        # If the string has the same format as the upper one, change it so it has the format as the lower one.
        if action_input.count("|") == 4:
            action_input = action_input.replace(" | ", "|")
            action_input = action_input.replace(" |", "|") 
            action_input = action_input.replace("| ", "|")

            # last new line
            if action_input[-1] == "\n":
                action_input = action_input[:-1]


        # Return the action and action input
        return AgentAction(tool=action, tool_input=action_input.strip(" ").strip('"'), log=llm_output)


def custom_agent_executor(tools, llm):
    # LLM chain consisting of the LLM and a prompt
    prompt = CustomPromptTemplate(
        template=template,
        tools=tools,
        # This omits the `agent_scratchpad`, `tools`, and `tool_names` variables because those are generated dynamically
        # This includes the `intermediate_steps` variable because that is needed
        input_variables=["input", "intermediate_steps"]
    )
    output_parser = CustomOutputParser()
    llm_chain = LLMChain(llm=llm, prompt=prompt)
    tool_names = [tool.name for tool in tools]
    agent = LLMSingleActionAgent(
        llm_chain=llm_chain, 
        output_parser=output_parser,
        stop=["\nObservation:"], 
        allowed_tools=tool_names
    )
    agent_executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools, verbose=True)
    return agent_executor
    

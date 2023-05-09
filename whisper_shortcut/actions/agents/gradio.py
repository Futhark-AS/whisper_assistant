from langchain.agents import initialize_agent
from langchain.chat_models import ChatOpenAI
from gradio_tools.tools import (StableDiffusionTool, StableDiffusionPromptGeneratorTool, SAMImageSegmentationTool, BarkTextToSpeechTool)

from langchain.memory import ConversationBufferMemory
from utils.agent_template import custom_agent_executor

llm = ChatOpenAI(temperature=0)
memory = ConversationBufferMemory(memory_key="chat_history")
tools = [StableDiffusionTool().langchain, StableDiffusionPromptGeneratorTool().langchain, SAMImageSegmentationTool().langchain, BarkTextToSpeechTool().langchain]


for tool in tools:
    print(tool)


agent = custom_agent_executor(tools, llm)
output = agent.run(input=("Please create a photo of A green field, full of cows, but use the promptgenerator tool to improve the prompt first. Then use the image segmentation tool to segment out all cows in the image. Then use the text to speech tool for generating speech of the image prompt."))

print(output)
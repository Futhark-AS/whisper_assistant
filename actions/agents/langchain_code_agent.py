# from actions.BaseAction import BaseAction
# import pyperclip
# from langchain.embeddings import OpenAIEmbeddings
# from langchain.text_splitter import TokenTextSplitter
# from langchain.vectorstores import Pinecone
# from langchain.agents import Tool, initialize_agent, AgentType
# import os
# import pinecone 
# from langchain.chat_models import ChatOpenAI
# from langchain.retrievers import ContextualCompressionRetriever
# from langchain.retrievers.document_compressors import LLMChainExtractor
# from config.shortcuts import super_key
# from pynput import keyboard

# from actions.agents.utils.agent_template import custom_agent_executor


# embeddings = OpenAIEmbeddings()

# # initialize pinecone
# pinecone_api_key = os.environ.get("PINECONE_API_KEY")
# pinecone.init(
#     api_key=pinecone_api_key,
#     environment="us-east1-gcp"  # next to api key in console
# )

# index_name = "langchain-docs"

# retriever = Pinecone.from_existing_index(index_name=index_name, embedding=embeddings).as_retriever(search_kwargs={"k": 7})

# llm = ChatOpenAI(temperature=0, model_name="gpt-3.5-turbo")
# llm.request_timeout = 120
# compressor = LLMChainExtractor.from_llm(llm)
# compression_retriever = ContextualCompressionRetriever(base_compressor=compressor, base_retriever=retriever)

# def pretty_print_docs(input_text):
#     docs = compression_retriever.get_relevant_documents(input_text)
#     text = "\n".join([f"Document {i+1}:\n\n" + d.page_content for i, d in enumerate(docs)])
#     return text





# tools = [
#     Tool(
#         name = "search",
#         func=pretty_print_docs,
#         description="Find relevant docs in the LangChain documentation based on your input."
#     ),
# ]

# chat = ChatOpenAI(model_name="gpt-4") 
# # agent = initialize_agent(tools, chat, agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION, verbose=True)
# agent = custom_agent_executor(tools, chat)
# def langchain_code_agent(input_text):
#     res = agent.run(input_text)
#     pyperclip.copy(res)


# langchain = super_key | {keyboard.KeyCode.from_char("5")}
# class LangchainCodeAgent(BaseAction):
#     def __init__(self):
#         config = {
#             "whisper_mode": "translate",
#             "use_clipboard_input": False,
#         }

#         super().__init__(
#             name="langchain_code_agent",
#             description="agent that can search the LangChain documentation and returns relevant code.",
#             action=langchain_code_agent,
#             shortcut=langchain,
#             config=config
#         )

# class LangchainCodeAgentInput(BaseAction):
#     def __init__(self):
#         config = {
#             "whisper_mode": "translate",
#             "use_clipboard_input": True,
#         }

#         super().__init__(
#             name="langchain_code_agent_input",
#             description="agent that can search the LangChain documentation and returns relevant code.",
#             action=langchain_code_agent,
#             shortcut=super_key | {keyboard.KeyCode.from_char("4")},
#             config=config
#         )
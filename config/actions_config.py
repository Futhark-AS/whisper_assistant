import logging
from actions.simple_gpt_action import SimpleGPTActionWithInput, Model, SimpleGPTAction
from actions.translate import WhisperTranslate
from actions.transcribe import WhisperTranscribe

from config.shortcuts import hotkey_translate, hotkey_transcribe, super_key, hotkey_gpt_with_input, hotkey_gpt_no_input

logger = logging.getLogger()

actions = [
    WhisperTranslate(shortcut=hotkey_translate),
    WhisperTranscribe(shortcut=hotkey_transcribe),
    SimpleGPTActionWithInput(
        shortcut=hotkey_gpt_with_input, whisper_mode="transcribe", model_name=Model.GROQ_LLAMA_3_8B
    ),
    SimpleGPTAction(
        shortcut=hotkey_gpt_no_input, model_name=Model.GROQ_LLAMA_3_70B
    ),
    # SimpleGPTActionWithInput(
    #     shortcut=super_key | {keyboard.KeyCode.from_char("4")}, whisper_mode="transcribe", model_name=Models.GPT4
    # ),
    # SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("5")}, model_name=Models.GPT3),
    # SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("6")}, model_name=Models.GPT4),
    # GoogleSearchReactAgent(shortcut=super_key | {keyboard.KeyCode.from_char("8")}),
    # SimpleGPTAction(shortcut=super_key | {keyboard.KeyCode.from_char("9")}, model_name=Models.GPT4, followup=True),
]

print("Ready")
# print all the available actions
for action in actions:
    print(f"{action.shortcut - super_key} - {action.name}")




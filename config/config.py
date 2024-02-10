import abc
import os
import openai
from audio.sounds import AudioPlayer
from dotenv import load_dotenv
import logging
import sys
from prompts.prompts import system_prompt_default
# Load environment variables from .env file
load_dotenv(override=True)


class Singleton(abc.ABCMeta, type):
    """
    Singleton metaclass for ensuring only one instance of a class.
    """

    _instances = {}

    def __call__(cls, *args, **kwargs):
        if cls not in cls._instances:
            cls._instances[cls] = super(
                Singleton, cls).__call__(
                *args, **kwargs)
        return cls._instances[cls]

def get_elevenlabs_api_keys():
    """
    Dynamically set the number of API keys based on how many exist in the .env file
    """
    keys = []
    index = 0
    while True:
        key_name = f"ELEVENLABS_API_KEY{index}"
        api_key = os.getenv(key_name)
        if api_key is not None:
            keys.append(api_key)
            index += 1
        else:
            break
    return keys

import logging

# Custom logging formatter to handle newlines properly
class NewlineFormatter(logging.Formatter):
    def format(self, record):
        message = super().format(record)
        return message.replace('\\n', '\n')

def setup_logging(log_file, debug):
    log_formatter = NewlineFormatter('%(asctime)s [%(levelname)s] %(message)s')

    file_handler = logging.FileHandler(log_file, encoding='utf-8')
    file_handler.setFormatter(log_formatter)
    file_handler.setLevel(logging.DEBUG)  # Set file_handler log level to DEBUG

    stream_handler = logging.StreamHandler(sys.stdout)
    stream_handler.setFormatter(log_formatter)
    if debug:
        stream_handler.setLevel(logging.DEBUG)  # Set stream_handler log level to DEBUG
    else:
        stream_handler.setLevel(logging.INFO)  # Set stream_handler log level to INFO

    root_logger = logging.getLogger()

    # Remove previous handlers
    for handler in root_logger.handlers[:]:
        handler.flush()
        handler.close()
        root_logger.removeHandler(handler)

    root_logger.setLevel(logging.DEBUG)
    root_logger.addHandler(file_handler)
    root_logger.addHandler(stream_handler)

class Config(metaclass=Singleton):
    """
    Configuration class to store the state of bools for different scripts access.
    """

    def __init__(self):
        self.debug = False

        self.save_mode = True
        self.preprocess_mode = True

        # Set up logging configuration
        self.log_file_name = './logs.log'
        setup_logging(self.log_file_name, self.debug)

        self.audioplayer = AudioPlayer()

        self.audioplayer.set_playback_speed(1.3)

        # Dynamically set the number of API keys based on how many exist in the .env file
        self.elevenlabs_api_keys = get_elevenlabs_api_keys()
        self.openai_api_key = os.getenv("OPENAI_API_KEY")

        self.gpt_model = "gpt-4"

        self.speak_mode = False
        # Initialize the OpenAI API client

        self.system_prompt = system_prompt_default
        self.whisper_system_prompt = ""

    def set_debug(self, value: bool):
        self.debug = value
        setup_logging(self.log_file_name, self.debug)

    def set_whisper_system_prompt(self, value: str):
        self.whisper_system_prompt = value

    # set system prompt
    def set_system_prompt(self, value: str):
        self.system_prompt = value


    def toggle_speak_mode(self):
        self.speak_mode = not self.speak_mode
from utils import print_price
import os
import pyaudio
import wave
import openai
import pyperclip
from pynput import keyboard
import threading
from datetime import datetime
import json
from pydub import AudioSegment
import traceback
from shortcuts import hotkey_stop, hotkey_cancel
from speak import say_text
from config import Config
import logging
from audio_processing import preprocess_audio, transcribe
import shutil
from langchain.callbacks import get_openai_callback 
from langchain.chat_models import ChatOpenAI
from prompts import (
    system_prompt_with_input,
    user_prompt_template,
    system_prompt_summarizer,
    system_prompt_default,
)
from actions_config import actions
from actions.BaseAction import BaseAction
from langchain.chat_models import ChatOpenAI
from langchain.schema import HumanMessage, SystemMessage
import pyperclip
from pynput import keyboard
from langchain.agents import load_tools
from langchain.agents import initialize_agent
from langchain.agents import AgentType
from langchain.llms import OpenAI
import socket

def set_ui_icon(state):
    global UI_STATE
    UI_STATE["mode"] = state

    update_ui_state()


def update_ui_state():
    title = UI_STATE["mode"]

    if cfg.speak_mode:
        title += " 🔊"

    if rumps_app is not None:
        rumps_app.title = title


cfg = Config()
logger = logging.getLogger()

WHISPER_PRICE = 0.006
GPT_3_PRICE = 0.0005
GPT_PROMPT_PRICE = 0.03
GPT_COMPLETION_PRICE = 0.06
AUDIO_FILE_NAME = "output.wav"
RAW_AUDIO_FILE_NAME = "raw_output.wav"

NOTIFICATION_SOUND = "/Users/skog/Documents/code/ai/agent-smith/whisper_shortcut/start_sound.wav"

UI_TXT = {
    "idle": "🧠",
    "recording": "🎙️",
    "transcribing": "📝",
    "processing": "🔁",
    "error": "❌",
}

recording = False
current_keys = set()
processing_thread = None
stop_action = False

next_action = None

rumps_app = None

socket_set_next_query = None

def upload(text, metadata):
    AUDIO_FILES_PATH = os.environ.get("AUDIO_FILES_PATH")

    if AUDIO_FILES_PATH is None:
        logger.info("No audio files path set, not uploading")
        return

    # Create a timestamped directory inside AUDIO_FILES_PATH
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    new_directory = os.path.join(AUDIO_FILES_PATH, f"upload_{timestamp}")

    if not os.path.exists(new_directory):
        os.makedirs(new_directory)

    # Copy the audio file to the new directory
    dest_path = os.path.join(new_directory, AUDIO_FILE_NAME)
    shutil.copyfile(AUDIO_FILE_NAME, dest_path)

    if cfg.preprocess_mode:
        # Copy the raw audio file to the new directory
        dest_path = os.path.join(new_directory, RAW_AUDIO_FILE_NAME)
        shutil.copyfile(RAW_AUDIO_FILE_NAME, dest_path)

    # Write the transcript to a text file in the new directory
    text_file_name = os.path.splitext(AUDIO_FILE_NAME)[0] + ".txt"
    dest_text_path = os.path.join(new_directory, text_file_name)

    with open(dest_text_path, "w", encoding="utf-8") as f:
        f.write(text)

    # Write the metadata to a file called metadata.txt in the new directory
    dest_metadata_path = os.path.join(new_directory, "metadata.txt")
    with open(dest_metadata_path, "w", encoding="utf-8") as f:
        f.write(metadata)

    logger.debug(f"Audio file, transcript, and metadata successfully uploaded to {new_directory}.")


def start_recording():
    global recording
    CHUNK = 1024
    FORMAT = pyaudio.paInt16
    CHANNELS = 1
    RATE = 44100  # Maybe try 48000?

    p = pyaudio.PyAudio()

    stream = p.open(format=FORMAT, channels=CHANNELS, rate=RATE, input=True, frames_per_buffer=CHUNK)

    logger.info("start recording...")
    frames = []
    while recording:
        data = stream.read(CHUNK)
        frames.append(data)
    logger.info("recording stopped")
    stream.stop_stream()
    stream.close()
    p.terminate()
    wf = wave.open(AUDIO_FILE_NAME, "wb")
    wf.setnchannels(CHANNELS)
    wf.setsampwidth(p.get_sample_size(FORMAT))
    wf.setframerate(RATE)
    wf.writeframes(b"".join(frames))
    wf.close()


def on_press(key):
    global current_keys, recording, processing_thread, stop_action, next_action
    current_keys.add(key)

    if processing_thread is None:
        if not recording:
            next_action = None
            for action in actions:
                if all([k in current_keys for k in action.shortcut]):
                    logger.info(f"{action.name} hotkey pressed")
                    logger.info(f"Action config: {action.config}")
                    next_action = action
                    break

            if next_action:
                recording = True
                processing_thread = threading.Thread(target=thread_main)
                processing_thread.start()

    elif all([k in current_keys for k in hotkey_stop]):
        logger.info("Stop hotkey pressed")
        on_stop_hotkey()

    elif all([k in current_keys for k in hotkey_cancel]):
        logger.info("Cancel hotkey pressed")
        recording = False
        stop_action = True


def on_stop_hotkey():
    global recording
    recording = False
    cfg.audioplayer.stop()

    set_ui_icon(UI_TXT["idle"])


def on_release(key):
    global current_keys
    if key in current_keys:
        current_keys.remove(key)


UI_STATE = {
    "mode": UI_TXT["idle"],
}

LAST_GPT_CONV = [{"role": "system", "content": cfg.system_prompt}]


def handle_socket_connection(data):
    global socket_set_next_query
    data = json.loads(data)
    action = data["action"]
    value = data["value"]

    if action == "sp": # Set system prompt
        if value == "default":
            cfg.system_prompt = system_prompt_default
        else:
            cfg.system_prompt = value

    elif action == "query":
        if value == "default":
            socket_set_next_query = None
        else:
            socket_set_next_query = value


def listen_for_connections(s):
    while True:
        c, addr = s.accept()
        logger.info(f"Got connection from {addr}")
        data = c.recv(1024)
        logger.info(f"Received data: {data}")
        handle_socket_connection(data)
        c.close()

def start_socket_listener():
    # Create a socket object
    s = socket.socket()
    
    # Bind to the port
    s.bind(('', 5555))

    # Now wait for client connection.
    s.listen(5)

    # Start a new thread to listen for connections
    connection_thread = threading.Thread(target=listen_for_connections, args=(s,))
    connection_thread.start()

def main(rumps_app2=None):
    global rumps_app
    rumps_app = rumps_app2

    set_ui_icon(UI_TXT["idle"])
    cfg.set_debug(False)

    # start listening on socket
    start_socket_listener()

    listener = keyboard.Listener(on_press=on_press, on_release=on_release)
    listener.start()
    listener.join()


def get_whisper_price():
    audio = AudioSegment.from_file(AUDIO_FILE_NAME)

    duration_minutes = len(audio) / (1000 * 60)  # Convert duration from milliseconds to minutes
    price = duration_minutes * WHISPER_PRICE

    return price


def thread_main():
    global processing_thread
    try:
        run_action()
    except Exception as e:
        logging.error("Error:", e)
        traceback.print_exc()
        set_ui_icon(UI_TXT["error"])
    finally:
        processing_thread = None

def run_action():
    global recording, processing_thread, stop_action, LAST_GPT_CONV, next_action

    # play sound
    cfg.audioplayer.play_audio_file(NOTIFICATION_SOUND)

    price = {}

    record_audio = next_action.config.get("record_input", True)

    if record_audio:
        set_ui_icon(UI_TXT["recording"])

        start_recording()

        set_ui_icon(UI_TXT["processing"])

        preprocess_audio(AUDIO_FILE_NAME, RAW_AUDIO_FILE_NAME)

        if stop_action:
            stop_action = False
            set_ui_icon(UI_TXT["idle"])
            return

        set_ui_icon(UI_TXT["transcribing"])

        whisper_prompt = next_action.config.get("whisper_prompt", None)

        print("record_audio", record_audio)

        text = transcribe(AUDIO_FILE_NAME, next_action.config["whisper_mode"], whisper_prompt)
        price["whisper"] = get_whisper_price()
        print_price(price)

        if cfg.save_mode:
            metadata = next_action.config
            upload(text, json.dumps(metadata))

        set_ui_icon(UI_TXT["processing"])

    logger.info(f"Running agent {next_action.name}")

    with get_openai_callback() as cb:
        next_action(text)
    print(cb)
    logger.info(f"Agent {next_action.name} finished")

    set_ui_icon(UI_TXT["idle"])


if __name__ == "__main__":
    main()

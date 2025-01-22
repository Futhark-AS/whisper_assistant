from config.config import Config
from config.env import initialize_env
initialize_env()
cfg = Config()
from utils import print_price
import os
import pyaudio
from pynput import keyboard
import threading
from datetime import datetime
import json
import traceback
from config.shortcuts import hotkey_stop, hotkey_cancel
import logging
from audio.audio_processing import transcribe
from prompts.prompts import (
    system_prompt_default,
)
from config.actions_config import actions
from pynput import keyboard
import socket
import numpy as np
import soundfile as sf
from io import BytesIO

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


logger = logging.getLogger()

WHISPER_PRICE = 0.0005
GPT_3_PRICE = 0.0005
GPT_PROMPT_PRICE = 0.03
GPT_COMPLETION_PRICE = 0.06
AUDIO_FILE_NAME = "output.wav"
RAW_AUDIO_FILE_NAME = "raw_output.wav"

# NOTIFICATION_SOUND = "../../../../audio/start_sound.wav"

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

def upload(text, metadata, processed_audio, sample_rate):
    AUDIO_FILES_PATH = os.environ.get("AUDIO_FILES_PATH")

    if AUDIO_FILES_PATH is None:
        logger.info("No audio files path set, not uploading")
        return

    # Create a timestamped directory inside AUDIO_FILES_PATH
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    new_directory = os.path.join(AUDIO_FILES_PATH, f"upload_{timestamp}")

    if not os.path.exists(new_directory):
        os.makedirs(new_directory)

    # Save the processed audio to the new directory
    dest_path = os.path.join(new_directory, AUDIO_FILE_NAME)
    sf.write(dest_path, processed_audio, sample_rate)

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
    FORMAT = pyaudio.paFloat32
    CHANNELS = 1
    RATE = 44100

    p = pyaudio.PyAudio()

    stream = p.open(format=FORMAT, channels=CHANNELS, rate=RATE, input=True, frames_per_buffer=CHUNK)

    logger.info("start recording...")
    frames = []
    try:    
        while recording:
            data = stream.read(CHUNK)
            frames.append(np.frombuffer(data, dtype=np.float32))
    except Exception as e:
        logger.error(f"Error recording audio: {e}")
    finally:
        logger.info("recording stopped")
        stream.stop_stream()
        stream.close()
        p.terminate()

    audio_data = np.concatenate(frames)
    return audio_data, RATE


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
        if value == "reset":
            socket_set_next_query = None
        else:
            socket_set_next_query = value

    elif action == "whisperprompt":
        if value == "reset":
            cfg.set_whisper_system_prompt("")
        # if value starts with add, add to prompt
        elif value.startswith("add"):
            prev = cfg.whisper_system_prompt
            cfg.set_whisper_system_prompt(prev + value[3:])
        elif value.startswith("set"):
            cfg.set_whisper_system_prompt(value[3:])

        print("Whisper system prompt:", cfg.whisper_system_prompt)

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


def get_whisper_price(duration_seconds):
    duration_minutes = duration_seconds / 60  # Convert seconds to minutes
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




import time

def run_action():
    global recording, processing_thread, stop_action, LAST_GPT_CONV, next_action

    price = {}
    timings = {}

    record_audio = next_action.config.get("record_input", True)

    if record_audio:
        set_ui_icon(UI_TXT["recording"])

        start_time = time.time()
        audio_data, sample_rate = start_recording()
        recording_time = time.time() - start_time
        timings["recording"] = recording_time

        set_ui_icon(UI_TXT["processing"])

        start_time = time.time()
        #processed_audio = preprocess_audio(audio_data, sample_rate)
        processed_audio = audio_data # TODO: For now just use the original audio since preprocessing is not working and groq is super cheap
        preprocessing_time = time.time() - start_time
        timings["preprocessing"] = preprocessing_time

        # Calculate lengths of original and processed audio
        original_length = len(audio_data) / sample_rate
        processed_length = len(processed_audio) / sample_rate

        if stop_action:
            stop_action = False
            set_ui_icon(UI_TXT["idle"])
            return

        set_ui_icon(UI_TXT["transcribing"])

        whisper_prompt = cfg.whisper_system_prompt

        # Convert processed audio to bytes
        audio_bytes = BytesIO()
        sf.write(audio_bytes, processed_audio, sample_rate, format='wav')
        audio_bytes.seek(0)


        start_time = time.time()
        text = transcribe(audio_bytes, next_action.config["whisper_mode"], whisper_prompt)
        transcription_time = time.time() - start_time
        timings["transcription"] = transcription_time

        price["whisper"] = get_whisper_price(processed_length)
        print_price(price)

        logger.info(f"Timings: Recording: {timings['recording']:.2f}s, Preprocessing: {timings['preprocessing']:.2f}s, Transcription: {timings['transcription']:.2f}s")
        logger.info(f"Audio lengths: Original: {original_length:.2f}s, Processed: {processed_length:.2f}s (Reduction: {(1 - processed_length/original_length)*100:.2f}%)")

        if cfg.save_mode:
            metadata = next_action.config
            upload(text, json.dumps(metadata), processed_audio, sample_rate)

        set_ui_icon(UI_TXT["processing"])

    logger.info(f"Running agent {next_action.name}")
    start_time = time.time()

    next_action(text)

    agent_time = time.time() - start_time
    timings["agent"] = agent_time

    logger.info(f"Agent {next_action.name} finished in {agent_time:.2f}s")
    logger.info(f"Total time: {sum(timings.values()):.2f}s")

    set_ui_icon(UI_TXT["idle"])

if __name__ == "__main__":
    main()

import os
os.environ["SERPER_API_KEY"] = "aac220712fb4df68bedadd83d7469b8e39d06596fd01a0e2d67ebadb2552ac47"
os.environ["SERPAPI_API_KEY"] = "aac220712fb4df68bedadd83d7469b8e39d06596fd01a0e2d67ebadb2552ac47"
os.environ["ZAPIER_NLA_API_KEY"] = ""

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
import requests
from audio_processing import preprocess_audio, transcribe
import shutil
from prompts import system_prompt_with_input, user_prompt_template, system_prompt_summarizer, system_prompt_without_input
from actions_config import actions


cfg = Config()
logger = logging.getLogger()

WHISPER_PRICE = 0.006
GPT_3_PRICE = 0.0005
GPT_PROMPT_PRICE = 0.03
GPT_COMPLETION_PRICE = 0.06
AUDIO_FILE_NAME = "output.wav"
RAW_AUDIO_FILE_NAME = "raw_output.wav"

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

def upload(text, metadata):
    AUDIO_FILES_PATH = os.environ.get("AUDIO_FILES_PATH")

    if AUDIO_FILES_PATH is None:
        logger.info("No audio files path set, not uploading")
        return

    # Create a timestamped directory inside AUDIO_FILES_PATH
    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
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
    text_file_name = os.path.splitext(AUDIO_FILE_NAME)[0] + '.txt'
    dest_text_path = os.path.join(new_directory, text_file_name)

    with open(dest_text_path, 'w', encoding='utf-8') as f:
        f.write(text)

    # Write the metadata to a file called metadata.txt in the new directory
    dest_metadata_path = os.path.join(new_directory, 'metadata.txt')
    with open(dest_metadata_path, 'w', encoding='utf-8') as f:
        f.write(metadata)

    logger.info(f"Audio file, transcript, and metadata successfully uploaded to {new_directory}.")

def start_recording():
    global recording
    CHUNK = 1024
    FORMAT = pyaudio.paInt16
    CHANNELS = 1
    RATE = 44100 # Maybe try 48000?

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
    wf = wave.open(AUDIO_FILE_NAME, 'wb')
    wf.setnchannels(CHANNELS)
    wf.setsampwidth(p.get_sample_size(FORMAT))
    wf.setframerate(RATE)
    wf.writeframes(b''.join(frames))
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
                processing_thread = threading.Thread(target=run_action)
                processing_thread.start()

    elif all([k in current_keys for k in hotkey_stop]):
        logger.info("Stop hotkey pressed")
        on_stop_hotkey()

    elif all([k in current_keys for k in hotkey_cancel]):
        logger.info("Cancel hotkey pressed")
        recording = False
        stop_action = True

def on_toggle_speak_mode():
    cfg.toggle_speak_mode()
    update_ui_state()

def on_stop_hotkey():
    global recording
    recording = False
    cfg.audioplayer.stop()

    set_status(UI_TXT["idle"])

def on_release(key):
    global current_keys
    if key in current_keys:
        current_keys.remove(key)
    
def on_tts_summarized_hotkey():
    thread = threading.Thread(target=speak_clipboard_summarized)
    thread.start()

def on_tts_hotkey():
    thread = threading.Thread(target=speak_clipboard)
    thread.start()

def on_summarize_current_hotkey():
    thread = threading.Thread(target=summarize_current_conv)
    thread.start()

def speak_clipboard():
    global use_gpt, use_gpt_input
    clipboard = pyperclip.paste()

    say_text(clipboard)

def summarize_current_conv():
    global LAST_GPT_CONV
    summary = summarize_text(json.dumps(LAST_GPT_CONV), "You are SummarizerGPT, and your job is to summarize a conversation. You are given the entire conversation, and you must extract the most important information and provide a summary for each content block. You will then only return the summary of the given conversation, no more.")

    LAST_GPT_CONV = [LAST_GPT_CONV[0]]
    LAST_GPT_CONV.append({
        "content": "Summary of our conversation this far: " + summary,
        "role": "user"
    })

    set_status(UI_TXT["idle"])
    

def speak_clipboard_summarized():
    global use_gpt, use_gpt_input
    clipboard = pyperclip.paste()

    summarized = summarize_text(clipboard, system_prompt_summarizer)

    say_text(summarized)



UI_STATE = {
    "mode": UI_TXT["idle"],
}

LAST_GPT_CONV = [{
    "role": "system",
    "content": system_prompt_without_input
}]

def set_status(state):
    global UI_STATE
    UI_STATE["mode"] = state

    update_ui_state()

def update_ui_state():
    title = UI_STATE["mode"]

    if cfg.speak_mode:
        title += " 🔊"



    if rumps_app is not None:
        rumps_app.title = title


def main(rumps_app2 = None):
    global rumps_app
    rumps_app = rumps_app2

    set_status(UI_TXT["idle"])
    cfg.set_debug(False)

    listener = keyboard.Listener(on_press=on_press, on_release=on_release)
    listener.start()
    listener.join()

def get_whisper_price():
    audio = AudioSegment.from_file(AUDIO_FILE_NAME)

    duration_minutes = len(audio) / (1000 * 60)  # Convert duration from milliseconds to minutes
    price = duration_minutes * WHISPER_PRICE

    return price

def convert_usd_to_nok(amount_usd):
    API_URL = "https://open.er-api.com/v6/latest/USD"

    try:
        response = requests.get(API_URL)
        response.raise_for_status()
        data = response.json()
        usd_to_nok_rate = data['rates']['NOK']
        amount_nok = amount_usd * usd_to_nok_rate
        return amount_nok
    except requests.exceptions.RequestException as e:
        print(f"Error fetching exchange rate: {e}")
        return None
    except Exception as e:
        print(f"Error: {e}")
        return None

def print_price(price):
    GREEN = '\033[32m'
    END_COLOR = '\033[0m'
    for key, value in price.items():
        print(f"{GREEN}{key.capitalize()}: ${value:.4f}{END_COLOR}")

    # Print total price
    total_price = sum(price.values())
    nok = convert_usd_to_nok(total_price)
    if nok is not None:
        print(f"{GREEN}Total: ${total_price:.4f} ({nok:.6f} NOK){END_COLOR}")
    else:
        print(f"{GREEN}Total: ${total_price:.4f}{END_COLOR}")



def summarize_text(text: str, system_prompt):
    messages = [
    {"role": "system", "content": system_prompt},
    {"role": "user", "content": text}
]
    set_status(UI_TXT["processing"])

    response = openai.ChatCompletion.create(
        messages=messages,
        model="gpt-3.5-turbo"
    )

    text = response.choices[0].message.content 

    price = {}

    price["gpt-3.5"] = response["usage"]["total_tokens"] / 1000 * GPT_3_PRICE

    print_price(price)

    return text

def run_action():
    global recording, processing_thread, stop_action, LAST_GPT_CONV, next_action
    price = {}
    # TODO: add error handling - giving feedback to user on error
    try:
        set_status(UI_TXT["recording"])

        start_recording()

        set_status(UI_TXT["processing"])

        preprocess_audio(AUDIO_FILE_NAME, RAW_AUDIO_FILE_NAME)

        if stop_action:
            stop_action = False
            set_status(UI_TXT["idle"])
            return

        set_status(UI_TXT["transcribing"])

        try:
            whisper_prompt = next_action.config["whisper_prompt"]
        except KeyError:
            whisper_prompt = None

        text = transcribe(AUDIO_FILE_NAME, next_action.config["whisper_mode"], whisper_prompt)

        price["whisper"] = get_whisper_price()

        # Store results of recording and transcription
        if cfg.save_mode:
            metadata = next_action.config

            # Store string version of metadata
            upload(text, json.dumps(metadata))

        set_status(UI_TXT["processing"])

        if next_action.config['use_clipboard_input']:
            inp = user_prompt_template % (pyperclip.paste(), text)
        else:
            inp = text


        logger.info(f"Running agent {next_action.name}")
        response = next_action(inp)
        logger.info(f"Agent {next_action.name} finished")
        

        #     price["gpt_prompt"] = response["usage"]["prompt_tokens"] / 1000 * GPT_PROMPT_PRICE
        #     price["gpt_completion"] = response["usage"]["completion_tokens"] / 1000 * GPT_COMPLETION_PRICE

        #     LAST_GPT_CONV = messages + [{"role": "assistant", "content": response.choices[0].message.content}]

        #     text = response.choices[0].message.content

        #     if cfg.speak_mode:
        #         say_text(text)

        #     print("Chat response:", text)

        # pyperclip.copy(text)

        # print_price(price)

        set_status(UI_TXT["idle"])

    except Exception as e:
        logging.error("Error:", e)
        traceback.print_exc()
        set_status(UI_TXT["error"])
    finally:
        processing_thread = None

if __name__ == "__main__":
    main()






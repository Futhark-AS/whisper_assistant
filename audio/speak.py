import os
import requests
from config.config import Config
import gtts
import logging

cfg = Config()

player = cfg.audioplayer

# TODO: Nicer names for these ids
voices = ["ErXwobaYiN019PkySvjV", "EXAVITQu4vr4xnSDxMaL"]

# Set to store the indices of failed API keys
failed_api_keys = set()

def eleven_labs_speech(text, voice_index=0):
    global failed_api_keys  # Use the global variable to remember failed keys

    # Use the voices list to select the voice based on the voice_index parameter
    voice_id = voices[voice_index]
    url = f"https://api.elevenlabs.io/v1/text-to-speech/{voice_id}"
    data = {"text": text}

    # Iterate through the available API keys
    # for i in len(cfg.elevenlabs_api_keys): # Max 99 keys
    for i, api_key in enumerate(cfg.elevenlabs_api_keys):
        # Skip the API key if it has previously failed
        if i in failed_api_keys:
            continue

        # Add the API key to the headers
        headers = {
            "Content-Type": "application/json",
            "xi-api-key": api_key
        }

        # Make the request
        response = requests.post(url, headers=headers, json=data)

        # Check the response status code
        if response.status_code == 401:
            # If the status code is 401, add the index to the set of failed keys
            logging.info(f"API key {i} failed with status code 401. Trying the next key.")
            failed_api_keys.add(i)
            continue
        elif response.status_code == 200:
            # If the status code is 200, the request was successful
            with open("speech.mpeg", "wb") as f:
                f.write(response.content)

            sound = player.load_audio("speech.mpeg")
            player.play(sound)
            os.remove("speech.mpeg")
            return True  # Indicate success
        else:
            # Handle other status codes as needed
            print(f"Request failed with status code {response.status_code}.")
            return False  # Indicate failure

    # If all available API keys have been tried and none worked, return False
    print("All available API keys failed or none were provided. Unable to process the request.")
    return False



def gtts_speech(text):
    tts = gtts.gTTS(text)
    tts.save("speech.mp3")

    sound = player.load_audio("speech.mp3")
    player.play(sound)
    os.remove("speech.mp3")

def say_text(text, voice_index=0):
    success = eleven_labs_speech(text, voice_index)
    if not success:
        gtts_speech(text)


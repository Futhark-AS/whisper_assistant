import logging
import os
import shutil
import traceback
import wave

from openai import OpenAI

client = OpenAI()
import pyaudio
from pydub import AudioSegment
from pydub.silence import split_on_silence

logger = logging.getLogger()


from groq import Groq

groq_client = Groq(timeout=10, max_retries=5)




# Start by making sure the `assemblyai` package is installed.
# If not, you can install it by running the following command:
# pip install -U assemblyai
#
# Note: Some macOS users may need to use `pip3` instead of `pip`.

import assemblyai as aai

# Replace with your API key
aai.settings.api_key = "39f064c52ceb442fab3a1fcba76e2ccf"
config_norwegian = aai.TranscriptionConfig(speech_model=aai.SpeechModel.nano, language_code="no")
config_english = aai.TranscriptionConfig(speech_model=aai.SpeechModel.best, language_code="en")

import librosa
import numpy as np
import soundfile as sf
import io

import logging
import numpy as np

logger = logging.getLogger(__name__)

import numpy as np
from pydub import AudioSegment
from pydub.silence import split_on_silence
import io
from io import BytesIO
import soundfile as sf

def preprocess_audio(audio_file_name, raw_audio_file_name, min_silence_len=1000, silence_thresh=-45, pause_between_chunks=200):
    # Load the audio file
    audio = AudioSegment.from_wav(audio_file_name)

    old_length = len(audio)

    # Concatenate non-silent chunks to create the output audio
    output_audio = AudioSegment.empty()

    # Split the audio on silence
    chunks = split_on_silence(audio, min_silence_len=min_silence_len, silence_thresh=silence_thresh)

    # Add back a small silence in the places where silence was removed
    pause_segment = AudioSegment.silent(duration=pause_between_chunks)

    for i, chunk in enumerate(chunks):
        output_audio += chunk
        if i < len(chunks) - 1:
            output_audio += pause_segment

    new_length = len(output_audio)

    logger.info(f"Preprocessed audio file from {old_length}ms to {new_length}ms. ({(old_length - new_length) / old_length * 100:.2f}% reduction)")

    # Copy the raw audio to a new file
    shutil.copy(audio_file_name, raw_audio_file_name)

    # Save the output audio file
    output_audio.export(audio_file_name, format="wav")

import io

def transcribe(audio_file, mode, whisper_prompt):
    logger.info("Transcribing audio...")

    # Convert audio_data to a file-like object
    audio_file.name = "audio.wav"  # Give a name to the file-like object

    if mode == "translate":
        transcript = groq_client.audio.translations.create(
            file=("audio.wav", audio_file),
            model="whisper-large-v3",
            prompt=whisper_prompt or "",
            response_format="json",
            temperature=0.0
        )
    elif mode == "transcribe":
        transcript = groq_client.audio.transcriptions.create(
            file=("audio.wav", audio_file),
            model="whisper-large-v3",
            prompt=whisper_prompt or "",
            response_format="json",
            temperature=0.0
        )
    else:
        logger.info("Invalid mode")
        return

    transcript_text = transcript.text

    logger.info(f"Transcribed result: {transcript_text}")
    return transcript_text

import logging
import os
from io import BytesIO
import soundfile as sf
from groq import Groq

logger = logging.getLogger(__name__)


class Transcriber:
    """Handles transcription of audio files to text using Groq Whisper API."""

    def __init__(self):
        """Initialize the transcriber with Groq client."""
        api_key = os.getenv("GROQ_API_KEY")
        if not api_key:
            raise ValueError("GROQ_API_KEY environment variable is not set")
        self.client = Groq(api_key=api_key, timeout=10, max_retries=5)
        self.model = "whisper-large-v3"

    def transcribe(self, audio_file_path, prompt="", language=None):
        """
        Transcribe an audio file to text.

        Args:
            audio_file_path: Path to the audio file
            prompt: Optional prompt to guide the transcription
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            str: Transcribed text
        """
        logger.debug(
            f"Transcribing audio file: {audio_file_path} (language: {language or 'inferred'})"
        )

        # Read audio file
        audio_data, sample_rate = sf.read(audio_file_path)

        # Convert to bytes for API
        audio_bytes = BytesIO()
        sf.write(audio_bytes, audio_data, sample_rate, format="wav")
        audio_bytes.seek(0)
        audio_bytes.name = "audio.wav"

        try:
            # Prepare arguments
            kwargs = {
                "file": ("audio.wav", audio_bytes),
                "model": self.model,
                "prompt": prompt or "",
                "response_format": "json",
                "temperature": 0.0,
            }

            # Only add language if specified
            if language:
                kwargs["language"] = language

            transcript = self.client.audio.transcriptions.create(**kwargs)

            transcript_text = transcript.text
            logger.debug(f"Transcription result: {transcript_text}")
            return transcript_text

        except Exception as e:
            logger.error(f"Error during transcription: {e}")
            raise

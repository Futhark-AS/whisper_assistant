import logging
import os
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

import soundfile as sf
from groq import Groq

logger = logging.getLogger(__name__)


# Known Whisper hallucinations to remove from transcriptions
HALLUCINATION_PATTERNS = [
    "Teksting av Nicolai Winther",
]


def _clean_hallucinations(text: str) -> tuple[str, int]:
    """Remove known Whisper hallucinations from text.

    Returns:
        tuple of (cleaned_text, total_removals)
    """
    total_removed = 0
    for pattern in HALLUCINATION_PATTERNS:
        count = text.count(pattern)
        if count > 0:
            text = text.replace(pattern, "")
            total_removed += count
    # Clean up any double spaces or newlines left behind
    while "  " in text:
        text = text.replace("  ", " ")
    while "\n\n\n" in text:
        text = text.replace("\n\n\n", "\n\n")
    return text.strip(), total_removed


def _format_timestamp(seconds: float) -> str:
    """Format seconds as HH:MM:SS or MM:SS timestamp."""
    hours = int(seconds // 3600)
    minutes = int((seconds % 3600) // 60)
    secs = int(seconds % 60)

    if hours > 0:
        return f"{hours}:{minutes:02d}:{secs:02d}"
    return f"{minutes}:{secs:02d}"


# Chunking configuration (in seconds)
# Only chunk audio files longer than this threshold
MIN_DURATION_FOR_CHUNKING_SECONDS = 10 * 60  # 10 minutes
# Size of each chunk when splitting long audio
CHUNK_DURATION_SECONDS = 10 * 60  # 10 minutes

# Video file extensions that we can extract audio from
VIDEO_EXTENSIONS = {".mp4", ".mkv", ".mov", ".avi", ".webm", ".m4v", ".flv", ".wmv"}


def _is_video_file(path: Path) -> bool:
    """Check if a file is a video file based on extension."""
    return path.suffix.lower() in VIDEO_EXTENSIONS


def _check_ffmpeg_available() -> bool:
    """Check if ffmpeg is available by running ffmpeg --version."""
    try:
        result = subprocess.run(
            ["ffmpeg", "-version"],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def _extract_audio_from_video(video_path: Path) -> Path:
    """
    Extract audio from video file to a temporary WAV file.

    Args:
        video_path: Path to the video file

    Returns:
        Path to the temporary WAV file (caller is responsible for cleanup)

    Raises:
        RuntimeError: If ffmpeg fails to extract audio
    """
    # Create temp file with .wav extension
    fd, temp_path = tempfile.mkstemp(suffix=".wav")
    os.close(fd)
    temp_wav = Path(temp_path)

    result = subprocess.run(
        [
            "ffmpeg",
            "-i",
            str(video_path),
            "-vn",  # No video
            "-acodec",
            "pcm_s16le",  # PCM 16-bit little-endian
            "-ar",
            "16000",  # 16kHz sample rate
            "-ac",
            "1",  # Mono
            "-y",  # Overwrite output
            str(temp_wav),
        ],
        capture_output=True,
    )

    if result.returncode != 0:
        # Clean up temp file on failure
        temp_wav.unlink(missing_ok=True)
        stderr = result.stderr.decode(errors="replace")
        raise RuntimeError(f"ffmpeg failed to extract audio: {stderr}")

    return temp_wav


class Transcriber:
    """Handles transcription of audio files to text using Groq Whisper API."""

    def __init__(
        self,
        min_duration_for_chunking: float | None = None,
        chunk_duration: float | None = None,
    ):
        """
        Initialize the transcriber with Groq client.

        Args:
            min_duration_for_chunking: Override minimum duration (seconds) before chunking.
                                       Defaults to MIN_DURATION_FOR_CHUNKING_SECONDS.
            chunk_duration: Override chunk size (seconds) when splitting.
                           Defaults to CHUNK_DURATION_SECONDS.
        """
        api_key = os.getenv("GROQ_API_KEY")
        if not api_key:
            raise ValueError("GROQ_API_KEY environment variable is not set")
        self.client = Groq(api_key=api_key, timeout=600, max_retries=3)
        self.model = "whisper-large-v3"
        self.min_duration_for_chunking = (
            min_duration_for_chunking
            if min_duration_for_chunking is not None
            else MIN_DURATION_FOR_CHUNKING_SECONDS
        )
        self.chunk_duration = (
            chunk_duration if chunk_duration is not None else CHUNK_DURATION_SECONDS
        )

    def _transcribe_audio_data(self, audio_data, sample_rate, prompt="", language=None):
        """
        Transcribe audio data (numpy array) to text.

        Args:
            audio_data: numpy array of audio samples
            sample_rate: sample rate of the audio
            prompt: Optional prompt to guide the transcription
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            str: Transcribed text
        """
        # Convert to bytes for API
        audio_bytes = BytesIO()
        sf.write(audio_bytes, audio_data, sample_rate, format="wav")
        audio_bytes.seek(0)
        audio_bytes.name = "audio.wav"

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
        return transcript.text

    def _split_audio_into_chunks(self, audio_data, sample_rate):
        """
        Split audio data into chunks of chunk_duration seconds.

        Args:
            audio_data: numpy array of audio samples
            sample_rate: sample rate of the audio

        Returns:
            list of numpy arrays, each containing a chunk of audio
        """
        samples_per_chunk = int(self.chunk_duration * sample_rate)
        total_samples = len(audio_data)

        chunks = []
        for start in range(0, total_samples, samples_per_chunk):
            end = min(start + samples_per_chunk, total_samples)
            chunks.append(audio_data[start:end])

        return chunks

    def transcribe(self, file_path, prompt="", language=None):
        """
        Transcribe an audio or video file to text. For long files, automatically splits
        into chunks and combines the transcriptions. Video files are automatically
        converted to audio using ffmpeg.

        Args:
            file_path: Path to the audio or video file
            prompt: Optional prompt to guide the transcription
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            str: Transcribed text

        Raises:
            RuntimeError: If file is a video and ffmpeg is not available
        """
        file_path = Path(file_path)
        temp_audio = None

        logger.debug(
            f"Transcribing file: {file_path} (language: {language or 'inferred'})"
        )

        # Handle video files by extracting audio first
        if _is_video_file(file_path):
            if not _check_ffmpeg_available():
                raise RuntimeError(
                    "To transcribe video files, ffmpeg must be installed. "
                    "Install with: brew install ffmpeg"
                )
            logger.info(f"Extracting audio from video: {file_path.name}")
            temp_audio = _extract_audio_from_video(file_path)
            audio_path = temp_audio
        else:
            audio_path = file_path

        try:
            return self._transcribe_audio(audio_path, prompt=prompt, language=language)
        finally:
            # Clean up temp file if we created one
            if temp_audio is not None and temp_audio.exists():
                temp_audio.unlink()

    def _transcribe_audio(self, audio_file_path, prompt="", language=None):
        """
        Internal method to transcribe an audio file. For long files, automatically
        splits into chunks and combines the transcriptions.

        Args:
            audio_file_path: Path to the audio file
            prompt: Optional prompt to guide the transcription
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            str: Transcribed text
        """
        # Read audio file
        audio_data, sample_rate = sf.read(audio_file_path)

        # Calculate duration
        duration_seconds = len(audio_data) / sample_rate
        duration_min = duration_seconds / 60
        logger.debug(f"Audio duration: {duration_seconds:.1f} seconds")
        print(f"Audio duration: {duration_min:.1f} minutes")

        # Check if we need to chunk
        if duration_seconds <= self.min_duration_for_chunking:
            # Short audio - transcribe directly
            try:
                transcript_text = self._transcribe_audio_data(
                    audio_data, sample_rate, prompt=prompt, language=language
                )
                logger.debug(f"Transcription result: {transcript_text}")
                # Clean hallucinations
                transcript_text, removed_count = _clean_hallucinations(transcript_text)
                if removed_count > 0:
                    print(
                        f"Removed {removed_count} hallucination(s) from transcription"
                    )
                return transcript_text
            except Exception as e:
                logger.error(f"Error during transcription: {e}")
                raise

        # Long audio - split into chunks and transcribe each
        logger.info(
            f"Audio is {duration_seconds:.1f}s, splitting into ~{self.chunk_duration}s chunks"
        )
        chunks = self._split_audio_into_chunks(audio_data, sample_rate)
        num_chunks = len(chunks)
        logger.info(f"Split into {num_chunks} chunks")
        print(
            f"Splitting into {num_chunks} chunks of ~{self.chunk_duration / 60:.0f} minutes each"
        )

        transcriptions = []
        current_prompt = prompt

        try:
            for i, chunk in enumerate(chunks):
                chunk_duration_secs = len(chunk) / sample_rate
                logger.info(
                    f"Transcribing chunk {i + 1}/{num_chunks} ({chunk_duration_secs:.1f}s)"
                )
                print(f"Transcribing chunk {i + 1}/{num_chunks}...")

                chunk_text = self._transcribe_audio_data(
                    chunk, sample_rate, prompt=current_prompt, language=language
                )
                transcriptions.append(chunk_text)
                logger.debug(f"Chunk {i + 1} transcription: {chunk_text[:100]}...")

                # Use the end of the previous transcription as context for the next
                # This helps maintain continuity across chunk boundaries
                if chunk_text:
                    # Take the last ~200 chars as context for the next chunk
                    current_prompt = (
                        chunk_text[-200:] if len(chunk_text) > 200 else chunk_text
                    )

            # Combine all transcriptions with chunk break markers
            if len(transcriptions) == 1:
                combined_text = transcriptions[0]
            else:
                parts = []
                chunk_time = 0.0
                intersection_contexts = []
                for i, text in enumerate(transcriptions):
                    parts.append(text)
                    chunk_time += len(chunks[i]) / sample_rate
                    # Add marker between chunks (not after the last one)
                    if i < len(transcriptions) - 1:
                        timestamp = _format_timestamp(chunk_time)
                        marker = f"\n\n[CHUNK BREAK @ {timestamp} - audio was split here, check for cut words/sentences]\n\n"
                        parts.append(marker)
                        # Store context around this intersection for later display
                        # Estimate ~15 sec of text: roughly 40 words/min speaking = ~10 words in 15 sec
                        # Average ~6 chars/word = ~60 chars, but be generous with ~300 chars
                        chars_for_15_sec = 300
                        end_of_prev = transcriptions[i][-chars_for_15_sec:].strip()
                        start_of_next = transcriptions[i + 1][:chars_for_15_sec].strip()
                        intersection_contexts.append(
                            (timestamp, end_of_prev, start_of_next)
                        )
                combined_text = "".join(parts)

                # Print intersection contexts at the end for easy copy-paste fixing
                if intersection_contexts:
                    print("\n" + "=" * 60)
                    print("CHUNK BOUNDARY CONTEXTS (±15 sec of text around each break)")
                    print(
                        "Copy-paste these if transcription looks weird at boundaries:"
                    )
                    print("=" * 60)
                    for timestamp, end_prev, start_next in intersection_contexts:
                        print(f"\n--- @ {timestamp} ---")
                        print(f"END OF PREVIOUS CHUNK:\n  ...{end_prev}")
                        print(f"\nSTART OF NEXT CHUNK:\n  {start_next}...")
                    print("\n" + "=" * 60 + "\n")

            # Clean hallucinations
            combined_text, removed_count = _clean_hallucinations(combined_text)
            if removed_count > 0:
                print(f"Removed {removed_count} hallucination(s) from transcription")

            logger.debug(f"Combined transcription length: {len(combined_text)} chars")
            return combined_text

        except Exception as e:
            logger.error(f"Error during transcription: {e}")
            raise

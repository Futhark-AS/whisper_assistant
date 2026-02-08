import logging
import os
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

import numpy as np
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
# Only chunk audio longer than this threshold
MIN_DURATION_FOR_CHUNKING_SECONDS = 5 * 60  # 5 minutes
# Size of each chunk when splitting long audio
CHUNK_DURATION_SECONDS = 4 * 60  # 4 minutes

# Video file extensions that we can extract audio from
VIDEO_EXTENSIONS = {".mp4", ".mkv", ".mov", ".avi", ".webm", ".m4v", ".flv", ".wmv"}

# Audio formats that require ffmpeg conversion (not natively supported by libsndfile)
FFMPEG_AUDIO_EXTENSIONS = {".m4a", ".aac", ".wma", ".opus", ".mp3"}


def _is_video_file(path: Path) -> bool:
    """Check if a file is a video file based on extension."""
    return path.suffix.lower() in VIDEO_EXTENSIONS


def _needs_ffmpeg_conversion(path: Path) -> bool:
    """Check if an audio file needs ffmpeg conversion."""
    return path.suffix.lower() in FFMPEG_AUDIO_EXTENSIONS


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
    """Extract audio from video file to a temporary WAV file.

    Args:
        video_path: Path to the video file

    Returns:
        Path to the temporary WAV file (caller is responsible for cleanup)

    Raises:
        RuntimeError: If ffmpeg fails to extract audio
    """
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
        temp_wav.unlink(missing_ok=True)
        stderr = result.stderr.decode(errors="replace")
        raise RuntimeError(f"ffmpeg failed to extract audio: {stderr}")

    return temp_wav


class Transcriber:
    """Handles transcription of audio files to text using Groq Whisper API."""

    def __init__(
        self,
        model: str = "whisper-large-v3",
        timeout: int = 60,
        min_duration_for_chunking: float | None = None,
        chunk_duration: float | None = None,
    ):
        """Initialize the transcriber with Groq client.

        Args:
            model: Whisper model name to use.
            timeout: HTTP timeout in seconds for API calls.
            min_duration_for_chunking: Override minimum duration (seconds) before chunking.
            chunk_duration: Override chunk size (seconds) when splitting.
        """
        api_key = os.getenv("GROQ_API_KEY")
        if not api_key:
            raise ValueError("GROQ_API_KEY environment variable is not set")
        self.client = Groq(api_key=api_key, timeout=timeout, max_retries=3)
        self.model = model
        self.min_duration_for_chunking = (
            min_duration_for_chunking
            if min_duration_for_chunking is not None
            else MIN_DURATION_FOR_CHUNKING_SECONDS
        )
        self.chunk_duration = (
            chunk_duration if chunk_duration is not None else CHUNK_DURATION_SECONDS
        )

    def transcribe_from_array(
        self,
        audio_data: np.ndarray,
        sample_rate: int,
        prompt: str = "",
        language: str | None = None,
    ) -> str:
        """Transcribe audio from a numpy array (in-memory hot path).

        Handles chunking for long audio automatically.

        Args:
            audio_data: numpy array of audio samples.
            sample_rate: sample rate of the audio.
            prompt: Optional prompt to guide the transcription.
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            Transcribed text.
        """
        duration_seconds = len(audio_data) / sample_rate
        logger.info(
            f"Audio duration: {duration_seconds / 60:.1f} minutes, "
            f"sample_rate={sample_rate}, samples={len(audio_data)}"
        )

        if duration_seconds <= self.min_duration_for_chunking:
            logger.debug(
                f"Audio under {self.min_duration_for_chunking}s threshold, transcribing directly"
            )
            text = self._transcribe_chunk(
                audio_data, sample_rate, prompt=prompt, language=language
            )
        else:
            chunks = self._split_into_chunks(audio_data, sample_rate)
            logger.info(
                f"Audio is {duration_seconds:.1f}s, split into {len(chunks)} chunks "
                f"of ~{self.chunk_duration / 60:.0f} min each"
            )
            text = self._transcribe_chunked(
                chunks, sample_rate, prompt=prompt, language=language
            )

        text, removed_count = _clean_hallucinations(text)
        if removed_count > 0:
            logger.info(f"Removed {removed_count} hallucination(s) from transcription")
        return text

    def transcribe(
        self,
        file_path: str | Path,
        prompt: str = "",
        language: str | None = None,
    ) -> str:
        """Transcribe an audio or video file to text.

        For long files, automatically splits into chunks and combines.
        Video files are converted to audio using ffmpeg.

        Args:
            file_path: Path to the audio or video file.
            prompt: Optional prompt to guide the transcription.
            language: Optional language code (e.g. "en", "nb"). If None, auto-detects.

        Returns:
            Transcribed text.

        Raises:
            RuntimeError: If file is a video and ffmpeg is not available.
        """
        file_path = Path(file_path)
        temp_audio: Path | None = None

        logger.debug(f"Transcribing file: {file_path} (language: {language or 'inferred'})")

        # Handle video files or unsupported audio formats by converting with ffmpeg
        if _is_video_file(file_path):
            if not _check_ffmpeg_available():
                raise RuntimeError(
                    "To transcribe video files, ffmpeg must be installed. "
                    "Install with: brew install ffmpeg"
                )
            logger.info(f"Extracting audio from video: {file_path.name}")
            temp_audio = _extract_audio_from_video(file_path)
            audio_path = temp_audio
        elif _needs_ffmpeg_conversion(file_path):
            if not _check_ffmpeg_available():
                raise RuntimeError(
                    f"To transcribe {file_path.suffix} files, ffmpeg must be installed. "
                    "Install with: brew install ffmpeg"
                )
            logger.info(f"Converting audio format: {file_path.name}")
            temp_audio = _extract_audio_from_video(file_path)
            audio_path = temp_audio
        else:
            audio_path = file_path

        try:
            audio_data, sample_rate = sf.read(audio_path)
            return self.transcribe_from_array(
                audio_data, sample_rate, prompt=prompt, language=language
            )
        finally:
            if temp_audio is not None and temp_audio.exists():
                temp_audio.unlink()

    # ── Internal methods ─────────────────────────────────────────────

    def _split_into_chunks(
        self, audio_data: np.ndarray, sample_rate: int
    ) -> list[np.ndarray]:
        """Split audio data into chunks of chunk_duration seconds."""
        samples_per_chunk = int(self.chunk_duration * sample_rate)
        total_samples = len(audio_data)

        chunks = []
        for start in range(0, total_samples, samples_per_chunk):
            end = min(start + samples_per_chunk, total_samples)
            chunks.append(audio_data[start:end])
        return chunks

    def _transcribe_chunk(
        self,
        audio_data: np.ndarray,
        sample_rate: int,
        prompt: str = "",
        language: str | None = None,
    ) -> str:
        """Transcribe a single audio chunk via the Groq Whisper API.

        Encodes audio as FLAC before uploading (~4x smaller than WAV).
        """
        audio_bytes = BytesIO()
        sf.write(audio_bytes, audio_data, sample_rate, format="flac")
        audio_bytes.seek(0)
        audio_bytes.name = "audio.flac"

        audio_size_mb = audio_bytes.getbuffer().nbytes / (1024 * 1024)
        audio_duration_sec = len(audio_data) / sample_rate
        logger.debug(
            f"Sending audio to API: {audio_duration_sec:.1f}s, {audio_size_mb:.2f}MB (FLAC)"
        )

        kwargs: dict[str, object] = {
            "file": ("audio.flac", audio_bytes),
            "model": self.model,
            "prompt": prompt or "",
            "response_format": "json",
            "temperature": 0.0,
        }
        if language:
            kwargs["language"] = language

        try:
            transcript = self.client.audio.transcriptions.create(**kwargs)
            return transcript.text
        except Exception as e:
            error_msg = str(e)
            if "413" in error_msg or "request_too_large" in error_msg.lower():
                logger.error(
                    f"Audio chunk too large for API ({audio_size_mb:.2f}MB, "
                    f"{audio_duration_sec:.1f}s). Consider reducing chunk_duration."
                )
                raise RuntimeError(
                    f"Audio chunk too large for Groq API ({audio_size_mb:.2f}MB, "
                    f"{audio_duration_sec:.1f}s). The API limit is ~25MB."
                ) from e
            logger.error(f"Transcription API error: {error_msg}")
            raise

    def _transcribe_chunked(
        self,
        chunks: list[np.ndarray],
        sample_rate: int,
        prompt: str = "",
        language: str | None = None,
    ) -> str:
        """Transcribe multiple audio chunks sequentially, using rolling context."""
        num_chunks = len(chunks)
        transcriptions: list[str] = []
        current_prompt = prompt

        for i, chunk in enumerate(chunks):
            chunk_duration_secs = len(chunk) / sample_rate
            logger.info(
                f"Transcribing chunk {i + 1}/{num_chunks} ({chunk_duration_secs:.1f}s)"
            )

            chunk_text = self._transcribe_chunk(
                chunk, sample_rate, prompt=current_prompt, language=language
            )
            transcriptions.append(chunk_text)
            logger.debug(f"Chunk {i + 1} transcription: {chunk_text[:100]}...")

            # Use the end of the previous transcription as context for the next
            if chunk_text:
                current_prompt = chunk_text[-200:] if len(chunk_text) > 200 else chunk_text

        return self._combine_transcriptions(transcriptions, chunks, sample_rate)

    def _combine_transcriptions(
        self,
        transcriptions: list[str],
        chunks: list[np.ndarray],
        sample_rate: int,
    ) -> str:
        """Combine chunk transcriptions with boundary markers."""
        if len(transcriptions) == 1:
            return transcriptions[0]

        parts: list[str] = []
        chunk_time = 0.0
        intersection_contexts: list[tuple[str, str, str]] = []

        for i, text in enumerate(transcriptions):
            parts.append(text)
            chunk_time += len(chunks[i]) / sample_rate

            if i < len(transcriptions) - 1:
                timestamp = _format_timestamp(chunk_time)
                marker = (
                    f"\n\n[CHUNK BREAK @ {timestamp} - "
                    f"audio was split here, check for cut words/sentences]\n\n"
                )
                parts.append(marker)

                chars_for_context = 300
                end_of_prev = transcriptions[i][-chars_for_context:].strip()
                start_of_next = transcriptions[i + 1][:chars_for_context].strip()
                intersection_contexts.append((timestamp, end_of_prev, start_of_next))

        combined_text = "".join(parts)

        if intersection_contexts:
            logger.info("Chunk boundary contexts (±15 sec of text around each break):")
            for timestamp, end_prev, start_next in intersection_contexts:
                logger.info(f"  @ {timestamp}: ...{end_prev[-80:]} | {start_next[:80:]}...")

        return combined_text

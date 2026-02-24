#!/usr/bin/env python3
"""
Speaker diarization for Norwegian interviews.

Usage: uv run diarization.py path/to/audio_or_video_file [output_path]

Requires environment variables:
- GROQ_API_KEY: For Whisper transcription
- HF_TOKEN: For pyannote.audio speaker diarization
"""

import os
import subprocess
import sys
import tempfile
from io import BytesIO
from pathlib import Path

import numpy as np
import soundfile as sf
from dotenv import load_dotenv
from groq import Groq
from pyannote.audio import Pipeline
import torch

# Load .env from current dir, then from quedo config
load_dotenv()
load_dotenv(Path.home() / ".config" / "quedo" / "config.env")

# Video file extensions that we can extract audio from
VIDEO_EXTENSIONS = {".mp4", ".mkv", ".mov", ".avi", ".webm", ".m4v", ".flv", ".wmv"}

# Chunking configuration (in seconds)
# Only chunk audio files longer than this threshold
MIN_DURATION_FOR_CHUNKING_SECONDS = 10 * 60  # 10 minutes
# Size of each chunk when splitting long audio
CHUNK_DURATION_SECONDS = 10 * 60  # 10 minutes


def get_device() -> torch.device:
    """Get the best available device (MPS for Mac, CUDA for Nvidia, else CPU)."""
    if torch.backends.mps.is_available():
        return torch.device("mps")
    elif torch.cuda.is_available():
        return torch.device("cuda")
    return torch.device("cpu")


def is_video_file(path: Path) -> bool:
    """Check if a file is a video file based on extension."""
    return path.suffix.lower() in VIDEO_EXTENSIONS


def check_ffmpeg_available() -> bool:
    """Check if ffmpeg is available."""
    try:
        result = subprocess.run(
            ["ffmpeg", "-version"],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def extract_audio_from_video(video_path: Path) -> Path:
    """
    Extract audio from video file to a temporary WAV file.

    Returns:
        Path to the temporary WAV file (caller is responsible for cleanup)
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


def split_audio_into_chunks(
    audio_data, sample_rate: int
) -> list[tuple[np.ndarray, float]]:
    """
    Split audio data into chunks of CHUNK_DURATION_SECONDS.

    Returns:
        List of (chunk_data, time_offset) tuples
    """
    samples_per_chunk = int(CHUNK_DURATION_SECONDS * sample_rate)
    total_samples = len(audio_data)

    chunks = []
    for start in range(0, total_samples, samples_per_chunk):
        end = min(start + samples_per_chunk, total_samples)
        time_offset = start / sample_rate
        chunks.append((audio_data[start:end], time_offset))

    return chunks


def transcribe_audio_data(
    audio_data, sample_rate: int, language: str = "no", prompt: str = ""
) -> list[dict]:
    """
    Transcribe audio data (numpy array) using Groq Whisper API.

    Returns:
        List of segments with 'start', 'end', 'text' keys
    """
    api_key = os.getenv("GROQ_API_KEY")
    if not api_key:
        raise ValueError("GROQ_API_KEY environment variable is not set")

    client = Groq(api_key=api_key, timeout=600, max_retries=3)

    # Convert to bytes for API
    audio_bytes = BytesIO()
    sf.write(audio_bytes, audio_data, sample_rate, format="wav")
    audio_bytes.seek(0)

    transcript = client.audio.transcriptions.create(
        file=("audio.wav", audio_bytes),
        model="whisper-large-v3",
        language=language,
        response_format="verbose_json",
        temperature=0.0,
        prompt=prompt,
    )

    segments = []
    for seg in transcript.segments:
        # Handle both dict and object access patterns
        if isinstance(seg, dict):
            segments.append(
                {
                    "start": seg["start"],
                    "end": seg["end"],
                    "text": seg["text"],
                }
            )
        else:
            segments.append(
                {
                    "start": seg.start,
                    "end": seg.end,
                    "text": seg.text,
                }
            )

    return segments


def diarize_audio_data(audio_data, sample_rate: int, pipeline: Pipeline) -> list[dict]:
    """
    Run speaker diarization on audio data (numpy array).

    Returns:
        List of speaker segments with 'speaker', 'start', 'end' keys
    """
    # Convert to mono if stereo
    if len(audio_data.shape) > 1:
        audio_data = audio_data.mean(axis=1)

    # Convert to torch tensor (channel, time) format
    waveform = torch.tensor(audio_data, dtype=torch.float32).unsqueeze(0)

    # Run diarization with 2 speakers for interview format
    diarization = pipeline(
        {"waveform": waveform, "sample_rate": sample_rate}, num_speakers=2
    )

    # Handle both Annotation return type and DiarizeOutput wrapper
    if hasattr(diarization, "itertracks"):
        annotation = diarization
    elif hasattr(diarization, "speaker_diarization"):
        annotation = diarization.speaker_diarization
    elif hasattr(diarization, "annotation"):
        annotation = diarization.annotation
    else:
        raise TypeError(f"Unknown diarization output type: {type(diarization)}")

    segments = []
    for turn, _, speaker in annotation.itertracks(yield_label=True):
        segments.append(
            {
                "speaker": speaker,
                "start": turn.start,
                "end": turn.end,
            }
        )

    return segments


def process_audio_chunked(
    audio_path: Path, language: str = "no"
) -> tuple[list[dict], list[dict]]:
    """
    Process audio file with chunking for long files.
    Runs both transcription and diarization on each chunk.

    Returns:
        Tuple of (whisper_segments, diarization_segments) with adjusted timestamps
    """
    # Read audio file
    audio_data, sample_rate = sf.read(audio_path)
    duration_seconds = len(audio_data) / sample_rate
    duration_min = duration_seconds / 60
    print(f"Audio duration: {duration_min:.1f} minutes")

    # Initialize pyannote pipeline once
    hf_token = os.getenv("HF_TOKEN")
    if not hf_token:
        raise ValueError(
            "HF_TOKEN environment variable is not set. "
            "Get your token at: https://huggingface.co/settings/tokens\n"
            "You also need to accept the license at: "
            "https://huggingface.co/pyannote/speaker-diarization-3.1"
        )

    device = get_device()
    print(f"Loading pyannote pipeline (device: {device})...")

    pipeline = Pipeline.from_pretrained(
        "pyannote/speaker-diarization-3.1",
        token=hf_token,
    )
    pipeline.to(device)

    # Check if we need to chunk
    if duration_seconds <= MIN_DURATION_FOR_CHUNKING_SECONDS:
        # Short audio - process directly
        print("\nTranscribing with Groq Whisper...")
        whisper_segments = transcribe_audio_data(audio_data, sample_rate, language)
        print(f"Transcription complete: {len(whisper_segments)} segments")

        print(f"\nRunning diarization on {audio_path.name}...")
        diarization_segments = diarize_audio_data(audio_data, sample_rate, pipeline)
        unique_speakers = len(set(s["speaker"] for s in diarization_segments))
        print(f"Diarization complete: found {unique_speakers} speakers")

        return whisper_segments, diarization_segments

    # Long audio - split into chunks
    chunks = split_audio_into_chunks(audio_data, sample_rate)
    num_chunks = len(chunks)
    print(
        f"\nSplitting into {num_chunks} chunks of ~{CHUNK_DURATION_SECONDS / 60:.0f} minutes each"
    )

    all_whisper_segments = []
    all_diarization_segments = []
    current_prompt = ""

    for i, (chunk_data, time_offset) in enumerate(chunks):
        chunk_duration_min = len(chunk_data) / sample_rate / 60
        print(
            f"\n--- Chunk {i + 1}/{num_chunks} ({chunk_duration_min:.1f} min, offset {time_offset / 60:.1f} min) ---"
        )

        # Transcribe chunk
        print("  Transcribing...")
        chunk_whisper = transcribe_audio_data(
            chunk_data, sample_rate, language, prompt=current_prompt
        )
        print(f"  Transcription: {len(chunk_whisper)} segments")

        # Adjust timestamps and add to results
        for seg in chunk_whisper:
            all_whisper_segments.append(
                {
                    "start": seg["start"] + time_offset,
                    "end": seg["end"] + time_offset,
                    "text": seg["text"],
                }
            )

        # Use end of this chunk's text as context for next chunk
        if chunk_whisper:
            last_text = " ".join(s["text"] for s in chunk_whisper[-3:])
            current_prompt = last_text[-200:] if len(last_text) > 200 else last_text

        # Diarize chunk
        print("  Diarizing...")
        chunk_diarization = diarize_audio_data(chunk_data, sample_rate, pipeline)
        print(f"  Diarization: {len(chunk_diarization)} segments")

        # Adjust timestamps and add to results
        for seg in chunk_diarization:
            all_diarization_segments.append(
                {
                    "speaker": seg["speaker"],
                    "start": seg["start"] + time_offset,
                    "end": seg["end"] + time_offset,
                }
            )

    print(
        f"\nTotal: {len(all_whisper_segments)} transcription segments, {len(all_diarization_segments)} diarization segments"
    )
    return all_whisper_segments, all_diarization_segments


def find_speaker_for_segment(
    seg_start: float,
    seg_end: float,
    diarization_segments: list[dict],
) -> str:
    """Find the speaker with maximum overlap for a given time range."""
    best_speaker = "UNKNOWN"
    best_overlap = 0.0

    for d_seg in diarization_segments:
        overlap_start = max(seg_start, d_seg["start"])
        overlap_end = min(seg_end, d_seg["end"])
        overlap = max(0.0, overlap_end - overlap_start)

        if overlap > best_overlap:
            best_overlap = overlap
            best_speaker = d_seg["speaker"]

    return best_speaker


def merge_transcription_with_diarization(
    whisper_segments: list[dict],
    diarization_segments: list[dict],
) -> list[dict]:
    """Merge Whisper segments with speaker labels from diarization."""
    labeled_segments = []

    for seg in whisper_segments:
        speaker = find_speaker_for_segment(
            seg["start"],
            seg["end"],
            diarization_segments,
        )
        labeled_segments.append(
            {
                "speaker": speaker,
                "start": seg["start"],
                "end": seg["end"],
                "text": seg["text"],
            }
        )

    return labeled_segments


def format_time(seconds: float) -> str:
    """Format seconds as MM:SS or HH:MM:SS."""
    hours = int(seconds // 3600)
    minutes = int((seconds % 3600) // 60)
    secs = int(seconds % 60)

    if hours > 0:
        return f"{hours}:{minutes:02d}:{secs:02d}"
    return f"{minutes}:{secs:02d}"


def format_diarized_transcript(labeled_segments: list[dict]) -> str:
    """
    Format labeled segments into a readable transcript.
    Consecutive segments from the same speaker are merged.
    """
    if not labeled_segments:
        return ""

    # Merge consecutive segments from same speaker
    merged = []
    current = None

    for seg in labeled_segments:
        if current is None:
            current = {
                "speaker": seg["speaker"],
                "start": seg["start"],
                "end": seg["end"],
                "text": seg["text"].strip(),
            }
        elif seg["speaker"] == current["speaker"]:
            current["end"] = seg["end"]
            current["text"] += " " + seg["text"].strip()
        else:
            merged.append(current)
            current = {
                "speaker": seg["speaker"],
                "start": seg["start"],
                "end": seg["end"],
                "text": seg["text"].strip(),
            }

    if current:
        merged.append(current)

    # Format output with timestamps
    lines = []
    for seg in merged:
        timestamp = format_time(seg["start"])
        lines.append(f"[{timestamp}] [{seg['speaker']}]: {seg['text']}")

    return "\n\n".join(lines)


def main():
    if len(sys.argv) < 2 or len(sys.argv) > 3:
        print("Usage: uv run diarization.py <audio_or_video_file> [output_path]")
        sys.exit(1)

    file_path = Path(sys.argv[1]).resolve()

    if not file_path.exists():
        print(f"Error: File not found: {file_path}")
        sys.exit(1)

    # Determine output path
    if len(sys.argv) == 3:
        output_path = Path(sys.argv[2]).resolve()
    else:
        output_path = file_path.with_suffix(".diarized.txt")

    temp_audio = None
    audio_path = file_path

    try:
        # Handle video files
        if is_video_file(file_path):
            if not check_ffmpeg_available():
                print(
                    "Error: ffmpeg is required for video files. Install with: brew install ffmpeg"
                )
                sys.exit(1)
            print(f"Extracting audio from video: {file_path.name}")
            temp_audio = extract_audio_from_video(file_path)
            audio_path = temp_audio

        # Run transcription and diarization (with chunking for long files)
        print("\n" + "=" * 60)
        print("Processing audio (transcription + diarization)")
        print("=" * 60)
        whisper_segments, diarization_segments = process_audio_chunked(
            audio_path, language="no"
        )

        # Merge results
        print("\n" + "=" * 60)
        print("Merging results")
        print("=" * 60)
        labeled_segments = merge_transcription_with_diarization(
            whisper_segments,
            diarization_segments,
        )

        # Format and print output
        transcript = format_diarized_transcript(labeled_segments)

        print("\n" + "=" * 60)
        print("DIARIZED TRANSCRIPT")
        print("=" * 60 + "\n")
        print(transcript)

        # Save to file
        output_path.write_text(transcript)
        print(f"\n\nSaved to: {output_path}")

    finally:
        # Clean up temp audio if we extracted from video
        if temp_audio is not None and temp_audio.exists():
            temp_audio.unlink()


if __name__ == "__main__":
    main()

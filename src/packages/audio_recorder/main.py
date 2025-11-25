import pyaudio
import numpy as np
import soundfile as sf
import logging
import os
import glob
from datetime import datetime

logger = logging.getLogger(__name__)


class AudioRecorder:
    """Handles audio recording from microphone to file."""

    def __init__(self, output_dir=None):
        """
        Initialize the audio recorder.

        Args:
            output_dir: Directory to save recordings. If None, uses current directory.
        """
        self.output_dir = output_dir or os.getcwd()
        self.recording = False
        self.p = None
        self.stream = None
        self.frames = []

        # Audio settings
        self.CHUNK = 1024
        self.FORMAT = pyaudio.paFloat32
        self.CHANNELS = 1
        self.RATE = 44100

    def start_recording(self):
        """
        Start recording audio. Blocks until stop_recording() is called.
        Saves to a file when stopped.

        Returns:
            str: Path to the saved audio file, or None if error
        """
        if self.recording:
            logger.debug("Recording already in progress")
            return None

        self.recording = True
        self.frames = []

        try:
            self.p = pyaudio.PyAudio()
            self.stream = self.p.open(
                format=self.FORMAT,
                channels=self.CHANNELS,
                rate=self.RATE,
                input=True,
                frames_per_buffer=self.CHUNK,
            )

            logger.debug("Recording started...")

            # Record in a loop until stop_recording() sets self.recording = False
            while self.recording:
                try:
                    data = self.stream.read(self.CHUNK, exception_on_overflow=False)
                    self.frames.append(np.frombuffer(data, dtype=np.float32))
                except Exception as e:
                    logger.error(f"Error during recording: {e}")
                    break

            logger.debug("Recording stopped")

        finally:
            # Cleanup stream and pyaudio
            if self.stream:
                try:
                    self.stream.stop_stream()
                    self.stream.close()
                except:
                    pass
            if self.p:
                try:
                    self.p.terminate()
                except:
                    pass

        # Save to file
        if not self.frames:
            logger.debug("No audio data recorded")
            return None

        audio_data = np.concatenate(self.frames)

        # Generate filename with timestamp
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"recording_{timestamp}.wav"
        filepath = os.path.join(self.output_dir, filename)

        # Save to file
        sf.write(filepath, audio_data, self.RATE)
        logger.debug(f"Audio saved to {filepath}")

        return filepath

    def cleanup_old_recordings(self, keep_filepath: str):
        """
        Delete all recording files except the one specified.

        Args:
            keep_filepath: Path to the file to keep (all others will be deleted)
        """
        try:
            # Find all recording files matching the pattern
            pattern = os.path.join(self.output_dir, "recording_*.wav")
            recording_files = glob.glob(pattern)

            # Filter out the file we want to keep
            files_to_delete = [
                f
                for f in recording_files
                if os.path.abspath(f) != os.path.abspath(keep_filepath)
            ]

            # Delete old recordings
            for file_path in files_to_delete:
                try:
                    os.remove(file_path)
                    logger.debug(f"Deleted old recording: {file_path}")
                except OSError as e:
                    logger.debug(f"Failed to delete {file_path}: {e}")

            if files_to_delete:
                logger.debug(f"Cleaned up {len(files_to_delete)} old recording(s)")

        except Exception as e:
            logger.error(f"Error during cleanup of old recordings: {e}", exc_info=True)

    def stop_recording(self):
        """
        Signal the recording to stop.
        The recording thread will finish and save the file.

        Returns:
            None (the file path is returned by start_recording())
        """
        if not self.recording:
            logger.debug("No recording in progress")
            return

        self.recording = False
        logger.debug("Stop signal sent")

    def is_recording(self):
        """Check if currently recording."""
        return self.recording

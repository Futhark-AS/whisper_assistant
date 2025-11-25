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

    def __init__(self, output_dir=None, notifier=None):
        """
        Initialize the audio recorder.

        Args:
            output_dir: Directory to save recordings. If None, uses current directory.
            notifier: Notifier instance for playing sounds. If None, creates a new one.
        """
        self.output_dir = output_dir or os.getcwd()
        self.recording = False
        self.p = None
        self.stream = None
        self.frames = []

        # Initialize notifier if not provided
        if notifier is None:
            from packages.notifications import Notifier

            self.notifier = Notifier()
        else:
            self.notifier = notifier

        # Audio settings
        self.CHUNK = 1024
        self.FORMAT = pyaudio.paFloat32
        self.CHANNELS = 1
        self.RATE = 44100

    def start_recording(self, output_path=None, notification_message="Recording..."):
        """
        Start recording audio. Blocks until stop_recording() is called.
        Saves to a file when stopped.

        Args:
            output_path: Optional full path to save the recording. If None, generates a filename.

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

            firstLoop = True
            while self.recording:
                try:
                    data = self.stream.read(self.CHUNK, exception_on_overflow=False)
                    self.frames.append(np.frombuffer(data, dtype=np.float32))

                    if firstLoop:
                        self.notifier.show_alert(
                            notification_message, "Whisper Assistant"
                        )
                        self.notifier.play_sound(
                            "/System/Library/Sounds/Hero.aiff", volume=25
                        )
                        firstLoop = False
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

        # Use provided output_path or generate filename
        if output_path:
            filepath = output_path
            # Ensure directory exists
            os.makedirs(os.path.dirname(filepath), exist_ok=True)
        else:
            # Generate filename with timestamp
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"recording_{timestamp}.wav"
            filepath = os.path.join(self.output_dir, filename)

        # Save to file
        sf.write(filepath, audio_data, self.RATE)
        logger.debug(f"Audio saved to {filepath}")

        return filepath

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

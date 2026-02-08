import logging
import threading

import numpy as np
import sounddevice as sd

logger = logging.getLogger(__name__)


class AudioRecorder:
    """Records audio from microphone using sounddevice callback API."""

    RATE = 16000
    CHANNELS = 1
    DTYPE = "int16"

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._recording = False
        self._stop_event = threading.Event()
        self._cancelled_event = threading.Event()
        self._frames: list[np.ndarray] = []
        # Pre-warm PortAudio device enumeration at startup
        sd.query_devices()

    def start_recording(self) -> tuple[np.ndarray, int] | None:
        """Block until stop_recording() or cancel_recording() is called.

        Returns (audio_array, sample_rate) or None if cancelled/error.
        """
        with self._lock:
            if self._recording:
                return None
            self._recording = True
            self._stop_event.clear()
            self._cancelled_event.clear()
            self._frames = []

        try:
            with sd.InputStream(
                samplerate=self.RATE,
                channels=self.CHANNELS,
                dtype=self.DTYPE,
                callback=self._audio_callback,
            ):
                self._stop_event.wait()  # Blocks until stop signal — zero CPU
        except Exception as e:
            logger.error(f"Recording stream error: {e}")
            return None
        finally:
            with self._lock:
                self._recording = False

        if self._cancelled_event.is_set():
            return None

        if not self._frames:
            return None

        audio = np.concatenate(self._frames)
        return (audio, self.RATE)

    def _audio_callback(
        self, indata: np.ndarray, frames: int, time: object, status: sd.CallbackFlags
    ) -> None:
        """Called by sounddevice from audio thread. Must be fast."""
        if status:
            logger.warning(f"Audio stream status: {status}")
        self._frames.append(indata.copy())

    def stop_recording(self) -> None:
        """Signal recording to stop. Non-blocking."""
        self._stop_event.set()

    def cancel_recording(self) -> None:
        """Signal recording to cancel (discard audio). Non-blocking."""
        self._cancelled_event.set()
        self._stop_event.set()

    def is_recording(self) -> bool:
        """Check if currently recording."""
        return self._recording

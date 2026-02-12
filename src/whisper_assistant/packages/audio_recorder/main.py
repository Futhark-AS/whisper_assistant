import logging
import threading
import time

import numpy as np
import sounddevice as sd

logger = logging.getLogger(__name__)


class AudioRecorder:
    """Records audio from microphone using sounddevice callback API."""

    RATE = 16000
    CHANNELS = 1
    DTYPE = "int16"

    _STREAM_CLOSE_TIMEOUT = 3.0  # seconds to wait for PortAudio stream close
    _OPEN_MAX_ATTEMPTS = 3
    _OPEN_RETRY_DELAYS = (0.25, 0.75)  # seconds between open attempts after resets
    _FALLBACK_SAMPLE_RATES = (48000, 44100)

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._recording = False
        self._stop_event = threading.Event()
        self._cancelled_event = threading.Event()
        self._frames: list[np.ndarray] = []
        self._active_sample_rate = self.RATE
        self._last_failure_kind: str | None = None
        self._last_failure_message: str | None = None
        self._consecutive_open_failures = 0
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
            self._active_sample_rate = self.RATE
            self._last_failure_kind = None
            self._last_failure_message = None
            self._stop_event.clear()
            self._cancelled_event.clear()
            self._frames = []

        stream = None
        try:
            stream = self._start_stream_with_recovery()
            self._stop_event.wait()
        except Exception as e:
            self._last_failure_kind = "stream_open_error"
            self._last_failure_message = str(e)
            self._consecutive_open_failures += 1
            self._log_audio_diagnostics("recording-open-failed")
            logger.error(f"Recording stream error: {e}")
            if self._consecutive_open_failures >= 2:
                logger.warning(
                    "Consecutive recording-open failures=%d; forcing aggressive backend reset",
                    self._consecutive_open_failures,
                )
                self._reset_portaudio_backend(aggressive=True)
            return None
        finally:
            self._close_stream(stream)
            with self._lock:
                self._recording = False

        if self._cancelled_event.is_set():
            self._last_failure_kind = "cancelled"
            self._last_failure_message = "recording cancelled by user"
            return None

        if not self._frames:
            self._last_failure_kind = "empty_audio"
            self._last_failure_message = "no audio frames captured"
            return None

        audio = np.concatenate(self._frames)
        self._last_failure_kind = None
        self._last_failure_message = None
        self._consecutive_open_failures = 0
        return (audio, self._active_sample_rate)

    def _start_stream_with_recovery(self) -> sd.InputStream:
        """Open/start InputStream with escalating recovery when backend is unstable."""
        errors: list[str] = []
        for attempt in range(self._OPEN_MAX_ATTEMPTS):
            if attempt > 0:
                aggressive = attempt >= 2
                mode = "aggressive" if aggressive else "soft"
                logger.warning(
                    "Retrying InputStream open (%d/%d) with %s backend reset",
                    attempt + 1,
                    self._OPEN_MAX_ATTEMPTS,
                    mode,
                )
                self._reset_portaudio_backend(aggressive=aggressive)
                delay = self._OPEN_RETRY_DELAYS[min(attempt - 1, len(self._OPEN_RETRY_DELAYS) - 1)]
                time.sleep(delay)

            stream: sd.InputStream | None = None
            try:
                # Refresh device list before each attempt (handles hot-plug/sleep-wake churn).
                sd.query_devices()
                stream, actual_rate = self._open_stream_with_fallback_configs()
                self._active_sample_rate = actual_rate
                if attempt == 1:
                    logger.info("Recovered recording stream after PortAudio reset")
                if attempt >= 2:
                    logger.info("Recovered recording stream after aggressive backend reset")
                return stream
            except Exception as exc:
                self._close_stream(stream)
                errors.append(str(exc))
                logger.warning(
                    "InputStream open attempt %d/%d failed: %s",
                    attempt + 1,
                    self._OPEN_MAX_ATTEMPTS,
                    exc,
                )
                self._log_audio_diagnostics(f"open-attempt-{attempt + 1}-failed")
                continue

        raise RuntimeError(
            "InputStream could not be started after retries; "
            f"attempt_errors={errors[-self._OPEN_MAX_ATTEMPTS:]}"
        )

    def _open_stream_with_fallback_configs(self) -> tuple[sd.InputStream, int]:
        """Open InputStream using preferred then fallback sample-rate configs."""
        candidate_rates: list[int] = [self.RATE]
        default_rate = self._get_default_input_sample_rate()
        if default_rate is not None and default_rate not in candidate_rates:
            candidate_rates.append(default_rate)
        for fallback in self._FALLBACK_SAMPLE_RATES:
            if fallback not in candidate_rates:
                candidate_rates.append(fallback)

        last_error: Exception | None = None
        for rate in candidate_rates:
            try:
                stream = sd.InputStream(
                    samplerate=rate,
                    channels=self.CHANNELS,
                    dtype=self.DTYPE,
                    callback=self._audio_callback,
                )
                stream.start()
                if rate != self.RATE:
                    logger.warning("Opened input stream with fallback sample_rate=%d", rate)
                return stream, int(rate)
            except Exception as exc:
                last_error = exc
                logger.warning("InputStream open failed for sample_rate=%d: %s", rate, exc)
                continue

        if last_error is not None:
            raise last_error
        raise RuntimeError("No sample-rate candidates were available for InputStream")

    @staticmethod
    def _get_default_input_sample_rate() -> int | None:
        """Get default input device sample rate, if available."""
        try:
            default_devices = sd.default.device
            if not isinstance(default_devices, (tuple, list)) or len(default_devices) < 1:
                return None
            input_idx = default_devices[0]
            if input_idx is None or int(input_idx) < 0:
                return None
            device = sd.query_devices(int(input_idx))
            rate = device.get("default_samplerate")
            if rate is None:
                return None
            return int(float(rate))
        except Exception:
            logger.debug("Failed to read default input sample rate", exc_info=True)
            return None

    @staticmethod
    def _reset_portaudio_backend(aggressive: bool = False) -> None:
        """Best-effort reset of sounddevice/PortAudio backend after device churn."""
        try:
            sd.stop()
        except Exception:
            logger.debug("sd.stop() failed during backend reset", exc_info=True)

        if aggressive:
            terminate = getattr(sd, "_terminate", None)
            initialize = getattr(sd, "_initialize", None)
            if callable(terminate) and callable(initialize):
                try:
                    terminate()
                    initialize()
                except Exception:
                    logger.debug(
                        "PortAudio terminate/init failed during backend reset", exc_info=True
                    )

        try:
            sd.query_devices()
        except Exception:
            logger.debug("sd.query_devices() failed after backend reset", exc_info=True)

    @staticmethod
    def _log_audio_diagnostics(context: str) -> None:
        """Log compact audio backend diagnostics useful for stream-open failures."""
        default_input = None
        default_output = None
        try:
            default_devices = sd.default.device
            if isinstance(default_devices, (tuple, list)) and len(default_devices) >= 2:
                default_input, default_output = default_devices[0], default_devices[1]
        except Exception:
            logger.debug("Failed reading default audio devices", exc_info=True)

        try:
            devices = sd.query_devices()
        except Exception:
            logger.warning("%s audio diagnostics: could not enumerate devices", context)
            return

        total_inputs = 0
        sample_inputs: list[str] = []
        for idx, device in enumerate(devices):
            max_in = int(device.get("max_input_channels", 0))
            if max_in <= 0:
                continue
            total_inputs += 1
            if len(sample_inputs) < 5:
                marker = "*" if default_input == idx else ""
                name = str(device.get("name", "?"))
                sample_inputs.append(f"{idx}{marker}:{name}:{max_in}ch")

        logger.warning(
            "%s audio diagnostics: default_in=%s default_out=%s input_devices=%d sample_inputs=%s",
            context,
            default_input,
            default_output,
            total_inputs,
            sample_inputs if sample_inputs else ["none"],
        )

    def _close_stream(self, stream: sd.InputStream | None) -> None:
        """Abort and close stream with a timeout so PortAudio can never hang us."""
        if stream is None:
            return
        try:
            stream.abort()
        except Exception:
            logger.debug("stream.abort() failed", exc_info=True)
        t = threading.Thread(target=self._do_close, args=(stream,), daemon=True)
        t.start()
        t.join(timeout=self._STREAM_CLOSE_TIMEOUT)
        if t.is_alive():
            logger.warning("PortAudio stream close timed out — abandoning stream")

    @staticmethod
    def _do_close(stream: sd.InputStream) -> None:
        try:
            stream.close()
        except Exception:
            logger.debug("stream.close() failed", exc_info=True)

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

    def get_last_failure_kind(self) -> str | None:
        """Get last recording failure classification."""
        return self._last_failure_kind

    def get_last_failure_message(self) -> str | None:
        """Get last recording failure message."""
        return self._last_failure_message

    def get_consecutive_open_failures(self) -> int:
        """Get count of consecutive stream-open failures."""
        return self._consecutive_open_failures

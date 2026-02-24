"""macOS permission checks for Accessibility, Input Monitoring, and Microphone."""

import logging
import subprocess
import sys

logger = logging.getLogger(__name__)

SYM_OK = "\u2713"
SYM_FAIL = "\u2717"
SYM_WARN = "\u26a0"

_SETTINGS_URLS: dict[str, str] = {
    "accessibility": "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    "input_monitoring": "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
    "microphone": "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
}


def check_accessibility() -> bool:
    """Check if Accessibility (AX) access is granted via AXIsProcessTrusted().

    Returns True on error or non-macOS (fail-open). Works on macOS 10.9+.
    """
    if sys.platform != "darwin":
        return True
    try:
        import ctypes
        import ctypes.util

        path = ctypes.util.find_library("ApplicationServices")
        if path is None:
            logger.debug("ApplicationServices framework not found, assuming granted")
            return True
        app_services = ctypes.cdll.LoadLibrary(path)
        app_services.AXIsProcessTrusted.restype = ctypes.c_bool
        return app_services.AXIsProcessTrusted()
    except Exception:
        logger.debug("Could not check Accessibility permission, assuming granted", exc_info=True)
        return True


def check_input_monitoring() -> bool:
    """Check if Input Monitoring access is granted via CGPreflightListenEventAccess().

    Returns True on error or non-macOS (fail-open). Works on macOS 10.15+.
    """
    if sys.platform != "darwin":
        return True
    try:
        import ctypes
        import ctypes.util

        path = ctypes.util.find_library("CoreGraphics")
        if path is None:
            logger.debug("CoreGraphics framework not found, assuming granted")
            return True
        core_graphics = ctypes.cdll.LoadLibrary(path)
        core_graphics.CGPreflightListenEventAccess.restype = ctypes.c_bool
        return core_graphics.CGPreflightListenEventAccess()
    except Exception:
        logger.debug("Could not check Input Monitoring permission, assuming granted", exc_info=True)
        return True


def check_microphone(timeout: float = 0.3) -> tuple[bool, str]:
    """Quick mic probe: record briefly and check for non-silence.

    Returns (True, "audio detected") if mic is live,
    (False, "silence detected - mic may be denied") if silent,
    or (True, "could not verify") on any error (fail-open).
    """
    try:
        import numpy as np
        import sounddevice as sd

        audio = sd.rec(
            int(timeout * 16000),
            samplerate=16000,
            channels=1,
            dtype="int16",
        )
        sd.wait()

        if audio.size == 0:
            return False, "no audio captured"
        if np.max(np.abs(audio)) > 10:
            return True, "audio detected"
        return False, "silence detected \u2014 mic may be denied"
    except Exception:
        logger.debug("Could not verify microphone access, assuming granted", exc_info=True)
        return True, "could not verify"


def check_all() -> dict[str, bool]:
    """Run all permission checks and return results."""
    mic_ok, _ = check_microphone()
    return {
        "accessibility": check_accessibility(),
        "input_monitoring": check_input_monitoring(),
        "microphone": mic_ok,
    }


def open_settings(pane: str) -> None:
    """Open the macOS System Settings pane for the given permission.

    pane: one of "accessibility", "input_monitoring", "microphone".
    """
    url = _SETTINGS_URLS.get(pane)
    if url is None:
        logger.warning(f"Unknown settings pane: {pane}")
        return
    subprocess.Popen(["open", url])

import logging
from pynput import keyboard
import threading

logger = logging.getLogger(__name__)


class KeyboardListener:
    """Handles global keyboard hotkey listening."""

    def __init__(self):
        """Initialize the keyboard listener."""
        self.hotkeys = []  # List of dicts: {'keys': set, 'object': keyboard.HotKey}
        self.listener = None
        self.running = False

    def register_hotkey(self, hotkey_set, callback):
        """
        Register a hotkey combination and its callback.

        Args:
            hotkey_set: Set of keyboard keys (e.g., {keyboard.Key.cmd, keyboard.KeyCode.from_char("2")})
            callback: Function to call when hotkey is pressed
        """

        # Wrap callback to catch exceptions and prevent listener from crashing
        def safe_callback():
            logger.debug(f"Hotkey triggered: {hotkey_set}")
            try:
                callback()
            except Exception as e:
                logger.error(f"Error in hotkey callback: {e}")

        # Create pynput HotKey object
        hk = keyboard.HotKey(hotkey_set, safe_callback)

        self.hotkeys.append({"keys": hotkey_set, "object": hk})
        logger.debug(f"Registered hotkey: {hotkey_set}")

    def unregister_hotkey(self, hotkey_set):
        """
        Unregister a hotkey combination.

        Args:
            hotkey_set: Set of keyboard keys (e.g., {keyboard.Key.cmd, keyboard.KeyCode.from_char("2")})
        """
        # Filter out the hotkey with matching keys
        # We match by converting to sorted tuple for stable comparison of sets
        target_tuple = tuple(sorted(hotkey_set, key=str))

        initial_len = len(self.hotkeys)
        self.hotkeys = [
            entry
            for entry in self.hotkeys
            if tuple(sorted(entry["keys"], key=str)) != target_tuple
        ]

        if len(self.hotkeys) < initial_len:
            logger.debug(f"Unregistered hotkey: {hotkey_set}")
        else:
            logger.debug(f"Hotkey not registered: {hotkey_set}")

    def _on_press(self, key):
        """Internal handler for key press events."""
        if not self.listener:
            return

        # Canonicalize the key (handles layout differences)
        canonical_key = self.listener.canonical(key)

        for entry in self.hotkeys:
            entry["object"].press(canonical_key)

    def _on_release(self, key):
        """Internal handler for key release events."""
        if not self.listener:
            return

        # Canonicalize the key
        canonical_key = self.listener.canonical(key)

        for entry in self.hotkeys:
            entry["object"].release(canonical_key)

    def start(self):
        """Start listening for keyboard events."""
        if self.running:
            logger.debug("Keyboard listener already running")
            return

        self.running = True
        self.listener = keyboard.Listener(
            on_press=self._on_press, on_release=self._on_release
        )
        self.listener.start()
        logger.debug("Keyboard listener started")

    def stop(self):
        """Stop listening for keyboard events."""
        if not self.running:
            return

        self.running = False
        if self.listener:
            self.listener.stop()
        logger.debug("Keyboard listener stopped")

    def join(self):
        """Wait for the listener thread to finish."""
        if self.listener:
            self.listener.join()

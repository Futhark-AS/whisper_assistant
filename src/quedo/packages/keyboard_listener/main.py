import logging
from collections.abc import Callable

from pynput import keyboard

logger = logging.getLogger(__name__)


class KeyboardListener:
    """Handles global keyboard hotkey listening."""

    def __init__(self) -> None:
        """Initialize the keyboard listener."""
        self.hotkeys: list[dict[str, object]] = []
        self.listener: keyboard.Listener | None = None
        self.running: bool = False

    def register_hotkey(
        self,
        hotkey_set: set[keyboard.Key | keyboard.KeyCode],
        callback: Callable[[], None],
    ) -> None:
        """Register a hotkey combination and its callback."""

        def safe_callback() -> None:
            logger.debug(f"Hotkey triggered: {hotkey_set}")
            try:
                callback()
            except Exception as e:
                logger.error(f"Error in hotkey callback: {e}")

        hk = keyboard.HotKey(hotkey_set, safe_callback)
        self.hotkeys.append({"keys": hotkey_set, "object": hk})
        logger.debug(f"Registered hotkey: {hotkey_set}")

    def unregister_hotkey(
        self, hotkey_set: set[keyboard.Key | keyboard.KeyCode]
    ) -> None:
        """Unregister a hotkey combination."""
        target_tuple = tuple(sorted(hotkey_set, key=str))

        initial_len = len(self.hotkeys)
        self.hotkeys = [
            entry
            for entry in self.hotkeys
            if tuple(sorted(entry["keys"], key=str)) != target_tuple  # type: ignore[arg-type]
        ]

        if len(self.hotkeys) < initial_len:
            logger.debug(f"Unregistered hotkey: {hotkey_set}")
        else:
            logger.debug(f"Hotkey not registered: {hotkey_set}")

    def _on_press(self, key: keyboard.Key | keyboard.KeyCode) -> None:
        """Internal handler for key press events."""
        if not self.listener:
            return

        canonical_key = self.listener.canonical(key)
        for entry in self.hotkeys:
            entry["object"].press(canonical_key)  # type: ignore[union-attr]

    def _on_release(self, key: keyboard.Key | keyboard.KeyCode) -> None:
        """Internal handler for key release events."""
        if not self.listener:
            return

        canonical_key = self.listener.canonical(key)
        for entry in self.hotkeys:
            entry["object"].release(canonical_key)  # type: ignore[union-attr]

    def start(self) -> None:
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

    def stop(self) -> None:
        """Stop listening for keyboard events."""
        if not self.running:
            return

        self.running = False
        if self.listener:
            self.listener.stop()
        logger.debug("Keyboard listener stopped")

    def join(self) -> None:
        """Wait for the listener thread to finish."""
        if self.listener:
            self.listener.join()

use crate::controller::events::ControllerEvent;
use crate::error::AppResult;

#[cfg(target_os = "macos")]
mod macos_hotkey {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

    use super::*;

    pub struct HotkeyController {
        manager: GlobalHotKeyManager,
        hotkey: HotKey,
    }

    impl HotkeyController {
        pub fn new(binding: &str) -> AppResult<Self> {
            let (modifiers, code) = parse_binding(binding)?;
            let manager = GlobalHotKeyManager::new().map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to initialize global hotkey manager: {error}"
                ))
            })?;
            let hotkey = HotKey::new(Some(modifiers), code);
            manager.register(hotkey).map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to register global hotkey `{binding}`: {error}"
                ))
            })?;

            Ok(Self { manager, hotkey })
        }

        pub fn drain_events(&self) -> Vec<ControllerEvent> {
            let mut events = Vec::new();
            while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                if event.id == self.hotkey.id() {
                    events.push(ControllerEvent::Toggle);
                }
            }
            events
        }
    }

    impl Drop for HotkeyController {
        fn drop(&mut self) {
            let _ = self.manager.unregister(self.hotkey);
        }
    }

    fn parse_binding(binding: &str) -> AppResult<(Modifiers, Code)> {
        let tokens = binding
            .split('+')
            .map(|part| part.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();

        if tokens.is_empty() {
            return Err(crate::error::AppError::Config(
                "hotkey binding cannot be empty".to_owned(),
            ));
        }

        let mut modifiers = Modifiers::empty();
        let mut key = None;

        for token in tokens {
            match token.as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                "shift" => modifiers |= Modifiers::SHIFT,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "cmd" | "command" | "super" => modifiers |= Modifiers::SUPER,
                "space" => key = Some(Code::Space),
                "a" => key = Some(Code::KeyA),
                "b" => key = Some(Code::KeyB),
                "c" => key = Some(Code::KeyC),
                "d" => key = Some(Code::KeyD),
                "e" => key = Some(Code::KeyE),
                "f" => key = Some(Code::KeyF),
                "g" => key = Some(Code::KeyG),
                "h" => key = Some(Code::KeyH),
                "i" => key = Some(Code::KeyI),
                "j" => key = Some(Code::KeyJ),
                "k" => key = Some(Code::KeyK),
                "l" => key = Some(Code::KeyL),
                "m" => key = Some(Code::KeyM),
                "n" => key = Some(Code::KeyN),
                "o" => key = Some(Code::KeyO),
                "p" => key = Some(Code::KeyP),
                "q" => key = Some(Code::KeyQ),
                "r" => key = Some(Code::KeyR),
                "s" => key = Some(Code::KeyS),
                "t" => key = Some(Code::KeyT),
                "u" => key = Some(Code::KeyU),
                "v" => key = Some(Code::KeyV),
                "w" => key = Some(Code::KeyW),
                "x" => key = Some(Code::KeyX),
                "y" => key = Some(Code::KeyY),
                "z" => key = Some(Code::KeyZ),
                _ => {
                    return Err(crate::error::AppError::Config(format!(
                        "unsupported hotkey token `{token}` in binding `{binding}`"
                    )));
                }
            }
        }

        let key = key.ok_or_else(|| {
            crate::error::AppError::Config(format!(
                "hotkey binding `{binding}` must include a key token (for example `Space`)"
            ))
        })?;

        Ok((modifiers, key))
    }
}

#[cfg(not(target_os = "macos"))]
pub struct HotkeyController;

#[cfg(not(target_os = "macos"))]
impl HotkeyController {
    pub fn new(_binding: &str) -> AppResult<Self> {
        Ok(Self)
    }

    pub fn drain_events(&self) -> Vec<ControllerEvent> {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
pub use macos_hotkey::HotkeyController;

#[cfg(test)]
mod tests {
    use super::HotkeyController;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_hotkey_is_noop() {
        let controller = HotkeyController::new("Ctrl+Shift+Space").expect("new");
        assert!(controller.drain_events().is_empty());
    }
}

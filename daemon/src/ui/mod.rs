pub mod hotkey;
pub mod notify;
pub mod tray;

use crate::controller::events::ControllerEvent;
use crate::controller::state::ControllerState;
use crate::error::AppResult;

pub struct UiFrontend {
    tray: tray::TrayController,
    hotkey: hotkey::HotkeyController,
}

impl UiFrontend {
    pub fn new(binding: &str) -> AppResult<Self> {
        Ok(Self {
            tray: tray::TrayController::new()?,
            hotkey: hotkey::HotkeyController::new(binding)?,
        })
    }

    pub fn drain_events(&self) -> Vec<ControllerEvent> {
        let mut events = self.tray.drain_events();
        events.extend(self.hotkey.drain_events());
        events
    }

    pub fn set_state(&self, state: &ControllerState) -> AppResult<()> {
        self.tray.set_state(state)
    }
}

pub use notify::Notifier;

#[cfg(test)]
mod tests {
    use super::UiFrontend;
    use crate::controller::state::ControllerState;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_frontend_behaves_as_noop() {
        let ui = UiFrontend::new("Ctrl+Shift+Space").expect("ui");
        assert!(ui.drain_events().is_empty());
        ui.set_state(&ControllerState::Idle).expect("set");
    }
}

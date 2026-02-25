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

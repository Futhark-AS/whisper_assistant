use crate::controller::events::ControllerEvent;
use crate::controller::state::ControllerState;
use crate::error::AppResult;

#[cfg(target_os = "macos")]
mod macos_tray {
    use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{TrayIcon, TrayIconBuilder};

    use super::*;

    pub struct TrayController {
        tray: TrayIcon,
        toggle_id: MenuId,
        doctor_id: MenuId,
        quit_id: MenuId,
    }

    impl TrayController {
        pub fn new() -> AppResult<Self> {
            let menu = Menu::new();

            let toggle_item = MenuItem::new("Toggle Recording", true, None);
            let doctor_item = MenuItem::new("Run Doctor", true, None);
            let quit_item = MenuItem::new("Quit", true, None);

            let toggle_id = toggle_item.id().clone();
            let doctor_id = doctor_item.id().clone();
            let quit_id = quit_item.id().clone();

            menu.append(&toggle_item).map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to append toggle menu item: {error}"
                ))
            })?;
            menu.append(&doctor_item).map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to append doctor menu item: {error}"
                ))
            })?;
            menu.append(&PredefinedMenuItem::separator())
                .map_err(|error| {
                    crate::error::AppError::Controller(format!(
                        "failed to append separator: {error}"
                    ))
                })?;
            menu.append(&quit_item).map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to append quit menu item: {error}"
                ))
            })?;

            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Quedo: idle")
                .build()
                .map_err(|error| {
                    crate::error::AppError::Controller(format!(
                        "failed to initialize tray icon: {error}"
                    ))
                })?;

            Ok(Self {
                tray,
                toggle_id,
                doctor_id,
                quit_id,
            })
        }

        pub fn drain_events(&self) -> Vec<ControllerEvent> {
            let mut events = Vec::new();
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == self.toggle_id {
                    events.push(ControllerEvent::Toggle);
                } else if event.id == self.doctor_id {
                    events.push(ControllerEvent::RunDoctor);
                } else if event.id == self.quit_id {
                    events.push(ControllerEvent::Shutdown);
                }
            }
            events
        }

        pub fn set_state(&self, state: &ControllerState) -> AppResult<()> {
            let label = match state {
                ControllerState::Idle => "Quedo: idle",
                ControllerState::Recording => "Quedo: recording",
                ControllerState::Processing => "Quedo: processing",
                ControllerState::Degraded(_) => "Quedo: degraded",
                ControllerState::Unavailable(_) => "Quedo: unavailable",
            };
            self.tray.set_tooltip(Some(label)).map_err(|error| {
                crate::error::AppError::Controller(format!(
                    "failed to update tray tooltip: {error}"
                ))
            })
        }
    }

    pub(super) use TrayController;
}

#[cfg(not(target_os = "macos"))]
pub struct TrayController;

#[cfg(not(target_os = "macos"))]
impl TrayController {
    pub fn new() -> AppResult<Self> {
        Ok(Self)
    }

    pub fn drain_events(&self) -> Vec<ControllerEvent> {
        Vec::new()
    }

    pub fn set_state(&self, _state: &ControllerState) -> AppResult<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use macos_tray::TrayController;

#[cfg(test)]
mod tests {
    use super::TrayController;
    use crate::controller::state::ControllerState;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_tray_is_noop() {
        let tray = TrayController::new().expect("new");
        assert!(tray.drain_events().is_empty());
        tray.set_state(&ControllerState::Idle).expect("set idle");
        tray.set_state(&ControllerState::Recording)
            .expect("set recording");
        tray.set_state(&ControllerState::Processing)
            .expect("set processing");
        tray.set_state(&ControllerState::Degraded("err".to_owned()))
            .expect("set degraded");
        tray.set_state(&ControllerState::Unavailable("err".to_owned()))
            .expect("set unavailable");
    }
}

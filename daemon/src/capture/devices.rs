#[cfg(target_os = "macos")]
use cpal::traits::{DeviceTrait, HostTrait};

use crate::error::{AppError, AppResult};

#[cfg(target_os = "macos")]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|error| AppError::Capture(format!("failed to enumerate input devices: {error}")))?;

    let mut names = Vec::new();
    for device in devices {
        let name = device
            .name()
            .map_err(|error| AppError::Capture(format!("failed to read device name: {error}")))?;
        names.push(name);
    }

    Ok(names)
}

#[cfg(not(target_os = "macos"))]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    Err(AppError::UnsupportedPlatform(
        "input device enumeration is only implemented for macOS in v1".to_owned(),
    ))
}

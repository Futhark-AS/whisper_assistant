#[cfg(target_os = "macos")]
use cpal::traits::{DeviceTrait, HostTrait};

use crate::error::{AppError, AppResult};

#[cfg(target_os = "macos")]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|error| {
        AppError::Capture(format!("failed to enumerate input devices: {error}"))
    })?;

    let mut names = Vec::new();
    for device in devices {
        let name = device
            .name()
            .map_err(|error| AppError::Capture(format!("failed to read device name: {error}")))?;
        names.push(name);
    }

    Ok(names)
}

#[cfg(target_os = "linux")]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    if which::which("arecord").is_ok() {
        let output = std::process::Command::new("arecord")
            .arg("-l")
            .output()
            .map_err(|error| {
                AppError::Capture(format!("failed to execute `arecord -l`: {error}"))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = stdout
            .lines()
            .filter(|line| line.contains("card "))
            .map(|line| line.trim().to_owned())
            .collect::<Vec<_>>();

        return Ok(devices);
    }

    if which::which("ffmpeg").is_ok() {
        return Ok(vec!["default (ffmpeg/alsa input)".to_owned()]);
    }

    Err(AppError::BinaryMissing {
        binary: "arecord or ffmpeg".to_owned(),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn list_input_devices() -> AppResult<Vec<String>> {
    Err(AppError::Capture(
        "input device enumeration is only implemented for macOS and Linux in v1".to_owned(),
    ))
}

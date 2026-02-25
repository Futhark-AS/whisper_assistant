use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy)]
pub struct CaptureWatchdogConfig {
    pub arming_timeout: Duration,
    pub stall_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct WatchdogSnapshot {
    pub armed: bool,
    pub stalled: bool,
    pub first_frame_seen: bool,
}

#[derive(Debug, Clone)]
pub struct MicrophoneCapture {
    preferred_device: Option<String>,
}

impl MicrophoneCapture {
    pub fn new(preferred_device: Option<String>) -> Self {
        Self { preferred_device }
    }

    #[cfg(target_os = "macos")]
    pub fn start_recording(
        &self,
        output_dir: &Path,
        watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        start_recording_macos(self.preferred_device.as_deref(), output_dir, watchdog)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn start_recording(
        &self,
        _output_dir: &Path,
        _watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        Err(AppError::UnsupportedPlatform(
            "microphone capture is only implemented for macOS in this environment".to_owned(),
        ))
    }
}

#[cfg(target_os = "macos")]
mod macos_capture {
    use std::fs::File;
    use std::io::BufWriter;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, Stream};

    use super::*;

    struct WatchdogState {
        first_frame_seen: AtomicBool,
        first_frame_at: Mutex<Option<Instant>>,
        last_frame_at: Mutex<Option<Instant>>,
        started_at: Instant,
    }

    impl WatchdogState {
        fn new() -> Self {
            Self {
                first_frame_seen: AtomicBool::new(false),
                first_frame_at: Mutex::new(None),
                last_frame_at: Mutex::new(None),
                started_at: Instant::now(),
            }
        }

        fn mark_frame(&self) {
            let now = Instant::now();
            if !self.first_frame_seen.swap(true, Ordering::SeqCst) {
                if let Ok(mut guard) = self.first_frame_at.lock() {
                    *guard = Some(now);
                }
            }
            if let Ok(mut guard) = self.last_frame_at.lock() {
                *guard = Some(now);
            }
        }

        fn snapshot(&self, cfg: CaptureWatchdogConfig) -> WatchdogSnapshot {
            let first_seen = self.first_frame_seen.load(Ordering::SeqCst);

            let armed = if first_seen {
                true
            } else {
                self.started_at.elapsed() <= cfg.arming_timeout
            };

            let stalled = if first_seen {
                match self.last_frame_at.lock() {
                    Ok(guard) => guard
                        .as_ref()
                        .map(|instant| instant.elapsed() > cfg.stall_timeout)
                        .unwrap_or(false),
                    Err(_) => true,
                }
            } else {
                false
            };

            WatchdogSnapshot {
                armed,
                stalled,
                first_frame_seen: first_seen,
            }
        }
    }

    pub struct ActiveRecording {
        wav_path: PathBuf,
        stream: Option<Stream>,
        writer: Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>,
        watchdog_cfg: CaptureWatchdogConfig,
        watchdog_state: Arc<WatchdogState>,
    }

    impl ActiveRecording {
        pub fn watchdog_snapshot(&self) -> WatchdogSnapshot {
            self.watchdog_state.snapshot(self.watchdog_cfg)
        }

        pub fn wav_path(&self) -> &Path {
            &self.wav_path
        }

        pub fn stop(mut self) -> AppResult<PathBuf> {
            let stream = self.stream.take();
            drop(stream);

            let mut writer_guard = self
                .writer
                .lock()
                .map_err(|_| AppError::Capture("wav writer lock poisoned".to_owned()))?;
            if let Some(writer) = writer_guard.take() {
                writer
                    .finalize()
                    .map_err(|error| AppError::Capture(format!("wav finalize failed: {error}")))?;
            }

            Ok(self.wav_path)
        }
    }

    pub fn start_recording_macos(
        preferred_device: Option<&str>,
        output_dir: &Path,
        watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        std::fs::create_dir_all(output_dir)?;
        let wav_path = output_dir.join(format!("capture-{}.wav", Uuid::new_v4()));

        let host = cpal::default_host();
        let device = select_device(&host, preferred_device)?;
        let input_config = device.default_input_config().map_err(|error| {
            AppError::Capture(format!("failed to resolve default input config: {error}"))
        })?;

        let wav_spec = hound::WavSpec {
            channels: input_config.channels(),
            sample_rate: input_config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = hound::WavWriter::create(&wav_path, wav_spec)
            .map_err(|error| AppError::Capture(format!("failed to create wav writer: {error}")))?;
        let writer = Arc::new(Mutex::new(Some(writer)));

        let watchdog_state = Arc::new(WatchdogState::new());

        let stream = build_stream(
            &device,
            input_config.sample_format(),
            &input_config.into(),
            writer.clone(),
            watchdog_state.clone(),
        )?;

        stream
            .play()
            .map_err(|error| AppError::Capture(format!("failed to start capture stream: {error}")))?;

        Ok(ActiveRecording {
            wav_path,
            stream: Some(stream),
            writer,
            watchdog_cfg: watchdog,
            watchdog_state,
        })
    }

    fn select_device(host: &cpal::Host, preferred_device: Option<&str>) -> AppResult<cpal::Device> {
        if let Some(preferred) = preferred_device {
            let devices = host.input_devices().map_err(|error| {
                AppError::Capture(format!("failed to list input devices: {error}"))
            })?;
            for device in devices {
                let name = device.name().map_err(|error| {
                    AppError::Capture(format!("failed to read input device name: {error}"))
                })?;
                if name == preferred {
                    return Ok(device);
                }
            }
        }

        host.default_input_device()
            .ok_or_else(|| AppError::Capture("no default microphone device found".to_owned()))
    }

    fn build_stream(
        device: &cpal::Device,
        sample_format: SampleFormat,
        stream_config: &cpal::StreamConfig,
        writer: Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>,
        watchdog_state: Arc<WatchdogState>,
    ) -> AppResult<Stream> {
        let error_callback = |error| {
            tracing::error!("cpal stream error: {error}");
        };

        match sample_format {
            SampleFormat::F32 => build_input_stream::<f32>(
                device,
                stream_config,
                writer,
                watchdog_state,
                error_callback,
            ),
            SampleFormat::I16 => build_input_stream::<i16>(
                device,
                stream_config,
                writer,
                watchdog_state,
                error_callback,
            ),
            SampleFormat::U16 => build_input_stream::<u16>(
                device,
                stream_config,
                writer,
                watchdog_state,
                error_callback,
            ),
            _ => Err(AppError::Capture(format!(
                "unsupported input sample format: {sample_format:?}"
            ))),
        }
    }

    fn build_input_stream<T>(
        device: &cpal::Device,
        stream_config: &cpal::StreamConfig,
        writer: Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>,
        watchdog_state: Arc<WatchdogState>,
        mut error_callback: impl FnMut(cpal::StreamError) + Send + 'static,
    ) -> AppResult<Stream>
    where
        T: cpal::SizedSample,
        i16: cpal::FromSample<T>,
    {
        let callback = move |data: &[T], _info: &cpal::InputCallbackInfo| {
            watchdog_state.mark_frame();
            if let Ok(mut guard) = writer.lock() {
                if let Some(writer) = guard.as_mut() {
                    for sample in data {
                        let as_i16: i16 = sample.to_sample::<i16>();
                        if let Err(error) = writer.write_sample(as_i16) {
                            tracing::error!("failed writing sample to wav: {error}");
                            break;
                        }
                    }
                }
            }
        };

        device
            .build_input_stream(stream_config, callback, move |error| error_callback(error), None)
            .map_err(|error| AppError::Capture(format!("failed to build input stream: {error}")))
    }

    pub(super) use ActiveRecording;
    pub(super) use start_recording_macos;
}

#[cfg(target_os = "macos")]
pub use macos_capture::ActiveRecording;

#[cfg(target_os = "macos")]
use macos_capture::start_recording_macos;

#[cfg(not(target_os = "macos"))]
pub struct ActiveRecording {
    wav_path: PathBuf,
}

#[cfg(not(target_os = "macos"))]
impl ActiveRecording {
    pub fn watchdog_snapshot(&self) -> WatchdogSnapshot {
        WatchdogSnapshot {
            armed: false,
            stalled: false,
            first_frame_seen: false,
        }
    }

    pub fn wav_path(&self) -> &Path {
        &self.wav_path
    }

    pub fn stop(self) -> AppResult<PathBuf> {
        Err(AppError::UnsupportedPlatform(
            "recording stop unavailable because capture is unsupported on this platform build"
                .to_owned(),
        ))
    }
}

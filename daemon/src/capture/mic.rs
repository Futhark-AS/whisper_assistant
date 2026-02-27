use std::path::{Path, PathBuf};
use std::time::Duration;

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

    #[cfg(target_os = "linux")]
    pub fn start_recording(
        &self,
        output_dir: &Path,
        watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        start_recording_linux(self.preferred_device.as_deref(), output_dir, watchdog)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn start_recording(
        &self,
        _output_dir: &Path,
        _watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        Err(AppError::Capture(
            "microphone capture is only implemented for macOS and Linux in this build".to_owned(),
        ))
    }
}

#[cfg(target_os = "macos")]
mod macos_capture {
    use std::fs::File;
    use std::io::BufWriter;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, Stream};
    use uuid::Uuid;

    use super::*;

    struct WatchdogState {
        first_frame_seen: AtomicBool,
        last_frame_at: Mutex<Option<Instant>>,
        started_at: Instant,
    }

    impl WatchdogState {
        fn new() -> Self {
            Self {
                first_frame_seen: AtomicBool::new(false),
                last_frame_at: Mutex::new(None),
                started_at: Instant::now(),
            }
        }

        fn mark_frame(&self) {
            let now = Instant::now();
            self.first_frame_seen.store(true, Ordering::SeqCst);
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

        stream.play().map_err(|error| {
            AppError::Capture(format!("failed to start capture stream: {error}"))
        })?;

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
            .build_input_stream(
                stream_config,
                callback,
                move |error| error_callback(error),
                None,
            )
            .map_err(|error| AppError::Capture(format!("failed to build input stream: {error}")))
    }
}

#[cfg(target_os = "macos")]
pub use macos_capture::ActiveRecording;

#[cfg(target_os = "macos")]
use macos_capture::start_recording_macos;

#[cfg(target_os = "linux")]
mod linux_capture {
    use std::io::Read;
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    use uuid::Uuid;

    use super::*;

    struct LinuxWatchdogState {
        first_frame_seen: bool,
        last_size: u64,
        last_growth_at: Instant,
    }

    pub struct ActiveRecording {
        wav_path: PathBuf,
        child: Child,
        started_at: Instant,
        watchdog_cfg: CaptureWatchdogConfig,
        watchdog_state: Mutex<LinuxWatchdogState>,
    }

    impl ActiveRecording {
        pub fn watchdog_snapshot(&self) -> WatchdogSnapshot {
            let now = Instant::now();
            let size = std::fs::metadata(&self.wav_path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);

            match self.watchdog_state.lock() {
                Ok(mut state) => {
                    if size > 44 {
                        if !state.first_frame_seen {
                            state.first_frame_seen = true;
                            state.last_growth_at = now;
                        }
                        if size > state.last_size {
                            state.last_size = size;
                            state.last_growth_at = now;
                        }
                    }

                    let armed = if state.first_frame_seen {
                        true
                    } else {
                        self.started_at.elapsed() <= self.watchdog_cfg.arming_timeout
                    };
                    let stalled = state.first_frame_seen
                        && now.duration_since(state.last_growth_at)
                            > self.watchdog_cfg.stall_timeout;

                    WatchdogSnapshot {
                        armed,
                        stalled,
                        first_frame_seen: state.first_frame_seen,
                    }
                }
                Err(_) => WatchdogSnapshot {
                    armed: false,
                    stalled: true,
                    first_frame_seen: false,
                },
            }
        }

        pub fn stop(mut self) -> AppResult<PathBuf> {
            terminate_recorder_gracefully(&mut self.child)?;
            validate_wav_header(&self.wav_path)?;
            Ok(self.wav_path)
        }
    }

    fn terminate_recorder_gracefully(child: &mut Child) -> AppResult<()> {
        let pid = child.id().to_string();
        match Command::new("kill").arg("-TERM").arg(&pid).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                tracing::warn!("failed to send SIGTERM to recorder process {pid}: {status}");
            }
            Err(error) => {
                tracing::warn!("failed to invoke kill -TERM for recorder process {pid}: {error}");
            }
        }

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait().map_err(|error| {
                AppError::Capture(format!(
                    "failed while waiting for recorder process termination: {error}"
                ))
            })? {
                Some(_status) => return Ok(()),
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                None => {
                    child.kill().map_err(|error| {
                        AppError::Capture(format!(
                            "failed to SIGKILL recorder process after timeout: {error}"
                        ))
                    })?;
                    child.wait().map_err(|error| {
                        AppError::Capture(format!(
                            "failed waiting for recorder process after SIGKILL: {error}"
                        ))
                    })?;
                    return Ok(());
                }
            }
        }
    }

    fn validate_wav_header(path: &Path) -> AppResult<()> {
        let metadata = std::fs::metadata(path).map_err(|error| {
            AppError::Capture(format!(
                "failed to stat recorder output {}: {error}",
                path.display()
            ))
        })?;
        if metadata.len() < 44 {
            return Err(AppError::Capture(format!(
                "recorded audio is not a valid WAV file (too short): {}",
                path.display()
            )));
        }
        if metadata.len() == 44 {
            return Err(AppError::Capture(format!(
                "recorded audio is empty (WAV header present but no PCM frames): {}",
                path.display()
            )));
        }

        let mut header = [0_u8; 12];
        let mut file = std::fs::File::open(path).map_err(|error| {
            AppError::Capture(format!(
                "failed to open recorder output for WAV validation {}: {error}",
                path.display()
            ))
        })?;
        file.read_exact(&mut header).map_err(|error| {
            AppError::Capture(format!(
                "failed to read WAV header from {}: {error}",
                path.display()
            ))
        })?;

        if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
            return Err(AppError::Capture(format!(
                "recorded audio is missing RIFF/WAVE header markers: {}",
                path.display()
            )));
        }

        Ok(())
    }

    pub fn start_recording_linux(
        preferred_device: Option<&str>,
        output_dir: &Path,
        watchdog: CaptureWatchdogConfig,
    ) -> AppResult<ActiveRecording> {
        std::fs::create_dir_all(output_dir)?;
        let wav_path = output_dir.join(format!("capture-{}.wav", Uuid::new_v4()));

        let child = if which::which("arecord").is_ok() {
            spawn_arecord(preferred_device, &wav_path)?
        } else if which::which("ffmpeg").is_ok() {
            spawn_ffmpeg(preferred_device, &wav_path)?
        } else {
            return Err(AppError::BinaryMissing {
                binary: "arecord or ffmpeg".to_owned(),
            });
        };

        Ok(ActiveRecording {
            wav_path,
            child,
            started_at: Instant::now(),
            watchdog_cfg: watchdog,
            watchdog_state: Mutex::new(LinuxWatchdogState {
                first_frame_seen: false,
                last_size: 0,
                last_growth_at: Instant::now(),
            }),
        })
    }

    fn spawn_arecord(preferred_device: Option<&str>, wav_path: &Path) -> AppResult<Child> {
        let mut command = Command::new("arecord");
        command
            .arg("-q")
            .arg("-f")
            .arg("S16_LE")
            .arg("-r")
            .arg("16000")
            .arg("-c")
            .arg("1");
        if let Some(device) = preferred_device {
            command.arg("-D").arg(device);
        }
        command
            .arg(wav_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| AppError::Capture(format!("failed to spawn arecord: {error}")))
    }

    fn spawn_ffmpeg(preferred_device: Option<&str>, wav_path: &Path) -> AppResult<Child> {
        let input_device = preferred_device.unwrap_or("default");
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "alsa",
                "-i",
                input_device,
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(wav_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| AppError::Capture(format!("failed to spawn ffmpeg capture: {error}")))
    }
}

#[cfg(target_os = "linux")]
pub use linux_capture::ActiveRecording;

#[cfg(target_os = "linux")]
use linux_capture::start_recording_linux;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub struct ActiveRecording;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl ActiveRecording {
    pub fn watchdog_snapshot(&self) -> WatchdogSnapshot {
        WatchdogSnapshot {
            armed: false,
            stalled: true,
            first_frame_seen: false,
        }
    }

    pub fn stop(self) -> AppResult<PathBuf> {
        Err(AppError::Capture(
            "recording stop unavailable because capture is unsupported on this platform build"
                .to_owned(),
        ))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::linux_capture::start_recording_linux;
    use super::CaptureWatchdogConfig;
    use crate::error::AppError;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    struct EnvVarGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old.as_ref() {
                std::env::set_var(self.key, old);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn write_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }

    fn watchdog(arming_ms: u64, stall_ms: u64) -> CaptureWatchdogConfig {
        CaptureWatchdogConfig {
            arming_timeout: Duration::from_millis(arming_ms),
            stall_timeout: Duration::from_millis(stall_ms),
        }
    }

    #[test]
    fn linux_uses_arecord_when_present() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let log = temp.path().join("invocations.log");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));
        let _log = EnvVarGuard::set("MOCK_LOG", log.to_str().expect("utf8"));

        let recorder_script = r#"#!/bin/sh
for arg in "$@"; do out="$arg"; done
echo "$0" >> "$MOCK_LOG"
printf "RIFF0000WAVE...................................." > "$out"
sleep 30
"#;
        write_script(&bin.join("arecord"), recorder_script);
        write_script(&bin.join("ffmpeg"), recorder_script);

        let recording =
            start_recording_linux(None, temp.path(), watchdog(500, 500)).expect("start");
        std::thread::sleep(Duration::from_millis(80));
        let wav_path = recording.stop().expect("stop");
        assert!(wav_path.exists());
        let log_text = fs::read_to_string(log).expect("log");
        assert!(log_text.contains("arecord"));
        assert!(!log_text.contains("ffmpeg"));
    }

    #[test]
    fn linux_falls_back_to_ffmpeg_when_arecord_missing() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let log = temp.path().join("invocations.log");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));
        let _log = EnvVarGuard::set("MOCK_LOG", log.to_str().expect("utf8"));

        let ffmpeg_script = r#"#!/bin/sh
for arg in "$@"; do out="$arg"; done
echo "$0" >> "$MOCK_LOG"
printf "RIFF0000WAVE...................................." > "$out"
sleep 30
"#;
        write_script(&bin.join("ffmpeg"), ffmpeg_script);

        let recording =
            start_recording_linux(None, temp.path(), watchdog(500, 500)).expect("start");
        std::thread::sleep(Duration::from_millis(80));
        let wav_path = recording.stop().expect("stop");
        assert!(wav_path.exists());
        let log_text = fs::read_to_string(log).expect("log");
        assert!(log_text.contains("ffmpeg"));
    }

    #[test]
    fn linux_errors_when_no_recorder_binary() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let empty_bin = temp.path().join("empty-bin");
        fs::create_dir_all(&empty_bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", empty_bin.to_str().expect("utf8"));

        let result = start_recording_linux(None, temp.path(), watchdog(500, 500));
        assert!(matches!(result, Err(AppError::BinaryMissing { .. })));
    }

    #[test]
    fn watchdog_arming_timeout_detection() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
for arg in "$@"; do out="$arg"; done
: > "$out"
sleep 30
"#,
        );

        let recording = start_recording_linux(None, temp.path(), watchdog(40, 500)).expect("start");
        std::thread::sleep(Duration::from_millis(80));
        let snapshot = recording.watchdog_snapshot();
        assert!(!snapshot.armed);
        assert!(!snapshot.first_frame_seen);
        let _ = recording.stop();
    }

    #[test]
    fn watchdog_stall_detection_after_initial_growth() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
for arg in "$@"; do out="$arg"; done
printf "RIFF0000WAVE...................................." > "$out"
printf "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" >> "$out"
sleep 30
"#,
        );

        let recording = start_recording_linux(None, temp.path(), watchdog(500, 50)).expect("start");
        std::thread::sleep(Duration::from_millis(60));
        let first = recording.watchdog_snapshot();
        assert!(first.armed);
        assert!(first.first_frame_seen);

        std::thread::sleep(Duration::from_millis(120));
        let snapshot = recording.watchdog_snapshot();
        assert!(snapshot.stalled);
        let _ = recording.stop();
    }

    #[test]
    fn stop_terminates_child_and_returns_wav_path() {
        let _guard = crate::test_support::lock_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).expect("mkdir");
        let _path = EnvVarGuard::set("PATH", bin.to_str().expect("utf8"));

        write_script(
            &bin.join("arecord"),
            r#"#!/bin/sh
for arg in "$@"; do out="$arg"; done
i=0
printf "RIFF0000WAVE...................................." > "$out"
while [ "$i" -lt 128 ]; do
  printf "a" >> "$out"
  i=$((i+1))
done
sleep 30
"#,
        );

        let recording =
            start_recording_linux(Some("default"), temp.path(), watchdog(500, 500)).expect("start");
        std::thread::sleep(Duration::from_millis(80));
        let wav = recording.stop().expect("stop");
        assert_eq!(wav.extension().and_then(|e| e.to_str()), Some("wav"));
    }
}

pub mod events;
pub mod queue;
pub mod state;

use std::thread;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use crate::bootstrap::AppPaths;
use crate::capture::{CaptureWatchdogConfig, MicrophoneCapture};
use crate::config::{AppConfig, OutputMode};
use crate::controller::events::{ControllerEvent, ControllerOutput};
use crate::controller::queue::SingleFlightQueue;
use crate::controller::state::ControllerState;
use crate::doctor::run_doctor;
use crate::error::{AppError, AppResult};
use crate::output::ClipboardOutput;
use crate::transcription::run_transcription_job;

#[derive(Debug, Clone)]
pub struct ControllerContext {
    pub config: AppConfig,
    pub paths: AppPaths,
}

pub fn run_controller_loop(
    context: ControllerContext,
    event_rx: Receiver<ControllerEvent>,
    event_tx: Sender<ControllerEvent>,
    output_tx: Sender<ControllerOutput>,
) -> AppResult<()> {
    let mut state = ControllerState::Idle;
    let capture = MicrophoneCapture::new(context.config.audio.device.clone());
    let mut active_recording = None;
    let mut recording_started_at: Option<Instant> = None;

    let mut queue = SingleFlightQueue::new(1);

    send_state(&output_tx, &state)?;

    loop {
        let event = event_rx
            .recv()
            .map_err(|_| AppError::ChannelClosed("controller event channel closed".to_owned()))?;

        match event {
            ControllerEvent::Toggle => match state {
                ControllerState::Idle | ControllerState::Degraded(_) => {
                    let watchdog_cfg = CaptureWatchdogConfig {
                        arming_timeout: std::time::Duration::from_millis(
                            context.config.audio.arming_timeout_ms,
                        ),
                        stall_timeout: std::time::Duration::from_millis(
                            context.config.audio.stall_timeout_ms,
                        ),
                    };

                    match capture.start_recording(&context.paths.cache_dir.join("capture"), watchdog_cfg)
                    {
                        Ok(recording) => {
                            active_recording = Some(recording);
                            recording_started_at = Some(Instant::now());
                            state = ControllerState::Recording;
                            send_state(&output_tx, &state)?;
                            send_notification(&output_tx, "Recording started")?;
                        }
                        Err(error) => {
                            let detail = format!("recording start failed: {error}");
                            state = ControllerState::Degraded(detail.clone());
                            send_state(&output_tx, &state)?;
                            send_notification(&output_tx, &detail)?;
                        }
                    }
                }
                ControllerState::Recording => {
                    if let Some(recording) = active_recording.take() {
                        recording_started_at = None;
                        match recording.stop() {
                            Ok(wav_path) => {
                                if let Err(error) = queue.enqueue(wav_path.clone()) {
                                    let detail = format!("unable to enqueue recording: {error}");
                                    state = ControllerState::Degraded(detail.clone());
                                    send_state(&output_tx, &state)?;
                                    send_notification(&output_tx, &detail)?;
                                } else {
                                    state = ControllerState::Processing;
                                    send_state(&output_tx, &state)?;
                                    spawn_next_job(
                                        &context,
                                        &mut queue,
                                        &event_tx,
                                        &output_tx,
                                        &wav_path,
                                    )?;
                                }
                            }
                            Err(error) => {
                                let detail = format!("failed to finalize recording: {error}");
                                state = ControllerState::Degraded(detail.clone());
                                send_state(&output_tx, &state)?;
                                send_notification(&output_tx, &detail)?;
                            }
                        }
                    }
                }
                ControllerState::Processing => {
                    send_notification(
                        &output_tx,
                        "Transcription already in progress; finishing current job.",
                    )?;
                }
            },
            ControllerEvent::RunDoctor => {
                let report = run_doctor(&context.paths, &context.config);
                output_tx
                    .send(ControllerOutput::DoctorReport(report))
                    .map_err(|_| {
                        AppError::ChannelClosed("controller output channel closed".to_owned())
                    })?;
            }
            ControllerEvent::Tick => {
                if let Some(recording) = active_recording.as_ref() {
                    let snapshot = recording.watchdog_snapshot();
                    if !snapshot.armed {
                        if let Some(recording) = active_recording.take() {
                            let _ = recording.stop();
                        }
                        recording_started_at = None;
                        state = ControllerState::Degraded(
                            "capture watchdog arming timeout exceeded".to_owned(),
                        );
                        send_state(&output_tx, &state)?;
                        send_notification(
                            &output_tx,
                            "Capture watchdog arming timeout exceeded; recording aborted.",
                        )?;
                    } else if snapshot.stalled {
                        if let Some(recording) = active_recording.take() {
                            let _ = recording.stop();
                        }
                        recording_started_at = None;
                        state = ControllerState::Degraded(
                            "capture watchdog stall detected".to_owned(),
                        );
                        send_state(&output_tx, &state)?;
                        send_notification(
                            &output_tx,
                            "Capture watchdog detected stalled input; recording aborted.",
                        )?;
                    }
                }

                if let (Some(started_at), Some(recording)) =
                    (recording_started_at.as_ref(), active_recording.take())
                {
                    if started_at.elapsed().as_secs() > context.config.audio.max_recording_seconds as u64
                    {
                        recording_started_at = None;
                        match recording.stop() {
                            Ok(wav_path) => {
                                let _ = queue.enqueue(wav_path.clone());
                                state = ControllerState::Processing;
                                send_state(&output_tx, &state)?;
                                spawn_next_job(
                                    &context,
                                    &mut queue,
                                    &event_tx,
                                    &output_tx,
                                    &wav_path,
                                )?;
                            }
                            Err(error) => {
                                let detail = format!("failed to finalize timed recording: {error}");
                                state = ControllerState::Degraded(detail.clone());
                                send_state(&output_tx, &state)?;
                                send_notification(&output_tx, &detail)?;
                            }
                        }
                    } else {
                        active_recording = Some(recording);
                    }
                }
            }
            ControllerEvent::TranscriptionFinished { wav_path, result } => {
                queue.mark_finished();

                if !context.config.audio.retain_audio && wav_path.exists() {
                    if let Err(error) = std::fs::remove_file(&wav_path) {
                        tracing::warn!("failed to remove capture artifact {}: {error}", wav_path.display());
                    }
                }

                match result {
                    Ok(result) => {
                        if context.config.output.mode == OutputMode::ClipboardOnly {
                            if let Err(error) = ClipboardOutput::write_text(&result.transcript) {
                                let detail = format!("clipboard output failed: {error}");
                                state = ControllerState::Degraded(detail.clone());
                                send_state(&output_tx, &state)?;
                                send_notification(&output_tx, &detail)?;
                                continue;
                            }
                        }

                        output_tx
                            .send(ControllerOutput::TranscriptReady(result))
                            .map_err(|_| {
                                AppError::ChannelClosed(
                                    "controller output channel closed".to_owned(),
                                )
                            })?;
                        state = ControllerState::Idle;
                        send_state(&output_tx, &state)?;
                        send_notification(&output_tx, "Transcription complete")?;
                    }
                    Err(error) => {
                        let detail = format!("transcription job failed: {error}");
                        state = ControllerState::Degraded(detail.clone());
                        send_state(&output_tx, &state)?;
                        send_notification(&output_tx, &detail)?;
                    }
                }
            }
            ControllerEvent::Shutdown => {
                if let Some(recording) = active_recording.take() {
                    let _ = recording.stop();
                }
                output_tx
                    .send(ControllerOutput::Stopped)
                    .map_err(|_| AppError::ChannelClosed("controller output channel closed".to_owned()))?;
                return Ok(());
            }
        }
    }
}

fn spawn_next_job(
    context: &ControllerContext,
    queue: &mut SingleFlightQueue,
    event_tx: &Sender<ControllerEvent>,
    output_tx: &Sender<ControllerOutput>,
    requested_wav_path: &std::path::Path,
) -> AppResult<()> {
    let wav_path = queue
        .start_next()
        .ok_or_else(|| AppError::Controller("queue was expected to have a job".to_owned()))?;

    if wav_path != requested_wav_path {
        return Err(AppError::Controller(format!(
            "queue scheduling mismatch: expected {}, got {}",
            requested_wav_path.display(),
            wav_path.display()
        )));
    }

    let db_path = context
        .config
        .history
        .db_path
        .clone()
        .unwrap_or_else(|| context.paths.history_db.clone());
    let transcription_cfg = context.config.transcription.clone();
    let output = output_tx.clone();
    let sender = event_tx.clone();

    thread::Builder::new()
        .name("quedo-transcription-job".to_owned())
        .spawn(move || {
            let result = run_transcription_job(wav_path.clone(), db_path, &transcription_cfg)
                .map_err(|error| error.to_string());
            if sender
                .send(ControllerEvent::TranscriptionFinished { wav_path, result })
                .is_err()
            {
                let _ = output.send(ControllerOutput::Notification(
                    "controller stopped before transcription completion could be delivered"
                        .to_owned(),
                ));
            }
        })
        .map_err(|error| AppError::Controller(format!("failed to spawn transcription thread: {error}")))?;

    Ok(())
}

fn send_state(output_tx: &Sender<ControllerOutput>, state: &ControllerState) -> AppResult<()> {
    output_tx
        .send(ControllerOutput::StateChanged(state.clone()))
        .map_err(|_| AppError::ChannelClosed("controller output channel closed".to_owned()))
}

fn send_notification(output_tx: &Sender<ControllerOutput>, message: &str) -> AppResult<()> {
    output_tx
        .send(ControllerOutput::Notification(message.to_owned()))
        .map_err(|_| AppError::ChannelClosed("controller output channel closed".to_owned()))
}

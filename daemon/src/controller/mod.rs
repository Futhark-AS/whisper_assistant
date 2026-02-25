pub mod events;
pub mod queue;
pub mod state;

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};

use crate::bootstrap::AppPaths;
use crate::capture::mic::WatchdogSnapshot;
use crate::capture::{CaptureWatchdogConfig, MicrophoneCapture};
use crate::config::{AppConfig, OutputMode, TranscriptionConfig};
use crate::controller::events::{ControllerEvent, ControllerOutput};
use crate::controller::queue::SingleFlightQueue;
use crate::controller::state::ControllerState;
use crate::doctor::{run_doctor, DoctorReport};
use crate::error::{AppError, AppResult};
use crate::output::ClipboardOutput;
use crate::transcription::{run_transcription_job, FrankenEngine};

#[derive(Debug, Clone)]
pub struct ControllerContext {
    pub config: AppConfig,
    pub paths: AppPaths,
}

enum WorkerMessage {
    Transcribe {
        wav_path: PathBuf,
        db_path: PathBuf,
        config: TranscriptionConfig,
    },
    Shutdown,
}

struct WorkerHandles {
    tx: Sender<WorkerMessage>,
    join: thread::JoinHandle<()>,
}

trait RecordingHandle: Send {
    fn watchdog_snapshot(&self) -> WatchdogSnapshot;
    fn stop(self: Box<Self>) -> AppResult<PathBuf>;
}

impl RecordingHandle for crate::capture::mic::ActiveRecording {
    fn watchdog_snapshot(&self) -> WatchdogSnapshot {
        self.watchdog_snapshot()
    }

    fn stop(self: Box<Self>) -> AppResult<PathBuf> {
        (*self).stop()
    }
}

pub fn run_controller_loop(
    context: ControllerContext,
    event_rx: Receiver<ControllerEvent>,
    event_tx: Sender<ControllerEvent>,
    output_tx: Sender<ControllerOutput>,
) -> AppResult<()> {
    let capture = MicrophoneCapture::new(context.config.audio.device.clone());

    let engine = FrankenEngine::new()?;
    let (worker_tx, worker_join) =
        spawn_transcription_worker(engine, event_tx.clone(), output_tx.clone())?;
    let worker = WorkerHandles {
        tx: worker_tx,
        join: worker_join,
    };

    run_controller_loop_with(
        context,
        event_rx,
        output_tx,
        move |output_dir, watchdog_cfg| {
            capture
                .start_recording(output_dir, watchdog_cfg)
                .map(|recording| Box::new(recording) as Box<dyn RecordingHandle>)
        },
        run_doctor,
        ClipboardOutput::write_text,
        worker,
    )
}

fn run_controller_loop_with<StartRecordingFn, RunDoctorFn, WriteClipboardFn>(
    context: ControllerContext,
    event_rx: Receiver<ControllerEvent>,
    output_tx: Sender<ControllerOutput>,
    mut start_recording: StartRecordingFn,
    mut doctor_runner: RunDoctorFn,
    write_clipboard: WriteClipboardFn,
    worker: WorkerHandles,
) -> AppResult<()>
where
    StartRecordingFn: FnMut(&Path, CaptureWatchdogConfig) -> AppResult<Box<dyn RecordingHandle>>,
    RunDoctorFn: FnMut(&AppPaths, &AppConfig) -> DoctorReport,
    WriteClipboardFn: Fn(&str) -> AppResult<()>,
{
    let mut state = ControllerState::Idle;
    let mut active_recording: Option<Box<dyn RecordingHandle>> = None;
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

                    match start_recording(&context.paths.cache_dir.join("capture"), watchdog_cfg) {
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
                                        &context, &mut queue, &worker.tx, &output_tx, &wav_path,
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
                let report = doctor_runner(&context.paths, &context.config);
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
                        state = ControllerState::Degraded(format!(
                            "capture watchdog arming timeout exceeded (first_frame_seen={})",
                            snapshot.first_frame_seen
                        ));
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
                        state = ControllerState::Degraded(format!(
                            "capture watchdog stall detected (first_frame_seen={})",
                            snapshot.first_frame_seen
                        ));
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
                    if started_at.elapsed().as_secs()
                        > context.config.audio.max_recording_seconds as u64
                    {
                        recording_started_at = None;
                        match recording.stop() {
                            Ok(wav_path) => {
                                if let Err(error) = queue.enqueue(wav_path.clone()) {
                                    let detail =
                                        format!("unable to enqueue timed recording stop: {error}");
                                    state = ControllerState::Degraded(detail.clone());
                                    send_state(&output_tx, &state)?;
                                    send_notification(&output_tx, &detail)?;
                                } else {
                                    state = ControllerState::Processing;
                                    send_state(&output_tx, &state)?;
                                    spawn_next_job(
                                        &context, &mut queue, &worker.tx, &output_tx, &wav_path,
                                    )?;
                                }
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
                        tracing::warn!(
                            "failed to remove capture artifact {}: {error}",
                            wav_path.display()
                        );
                    }
                }

                match result {
                    Ok(result) => {
                        if context.config.output.mode == OutputMode::ClipboardOnly {
                            if let Err(error) = write_clipboard(&result.transcript) {
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

                let _ = worker.tx.send(WorkerMessage::Shutdown);
                let _ = worker.join.join();

                output_tx.send(ControllerOutput::Stopped).map_err(|_| {
                    AppError::ChannelClosed("controller output channel closed".to_owned())
                })?;
                return Ok(());
            }
        }
    }
}

fn spawn_transcription_worker(
    engine: FrankenEngine,
    event_tx: Sender<ControllerEvent>,
    output_tx: Sender<ControllerOutput>,
) -> AppResult<(Sender<WorkerMessage>, thread::JoinHandle<()>)> {
    let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerMessage>();

    let join_handle = thread::Builder::new()
        .name("quedo-transcription-worker".to_owned())
        .spawn(move || {
            while let Ok(message) = worker_rx.recv() {
                match message {
                    WorkerMessage::Transcribe {
                        wav_path,
                        db_path,
                        config,
                    } => {
                        let result = run_transcription_job(&engine, wav_path.clone(), db_path, &config)
                            .map_err(|error| error.to_string());

                        if event_tx
                            .send(ControllerEvent::TranscriptionFinished { wav_path, result })
                            .is_err()
                        {
                            let _ = output_tx.send(ControllerOutput::Notification(
                                "controller stopped before transcription completion could be delivered"
                                    .to_owned(),
                            ));
                            break;
                        }
                    }
                    WorkerMessage::Shutdown => break,
                }
            }
        })
        .map_err(|error| {
            AppError::Controller(format!("failed to spawn transcription worker: {error}"))
        })?;

    Ok((worker_tx, join_handle))
}

fn spawn_next_job(
    context: &ControllerContext,
    queue: &mut SingleFlightQueue,
    worker_tx: &Sender<WorkerMessage>,
    output_tx: &Sender<ControllerOutput>,
    requested_wav_path: &Path,
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

    worker_tx
        .send(WorkerMessage::Transcribe {
            wav_path,
            db_path,
            config: transcription_cfg,
        })
        .map_err(|_| {
            let _ = output_tx.send(ControllerOutput::Notification(
                "transcription worker channel is closed".to_owned(),
            ));
            AppError::Controller("transcription worker channel closed".to_owned())
        })
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

#[cfg(test)]
mod tests {
    use super::{
        run_controller_loop_with, send_notification, send_state, spawn_next_job, ControllerContext,
        RecordingHandle, SingleFlightQueue, WorkerHandles, WorkerMessage,
    };
    use crate::bootstrap::paths::AppPaths;
    use crate::capture::mic::WatchdogSnapshot;
    use crate::config::schema::AppConfig;
    use crate::config::OutputMode;
    use crate::controller::events::{ControllerEvent, ControllerOutput};
    use crate::controller::state::ControllerState;
    use crate::doctor::report::{DoctorReport, DoctorState};
    use crate::error::{AppError, AppResult};
    use crate::transcription::TranscriptResult;
    use crossbeam_channel::{Receiver, Sender};
    use franken_whisper::BackendKind;
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    struct FakeRecording {
        wav_path: PathBuf,
        snapshot: WatchdogSnapshot,
        stop_count: Arc<AtomicUsize>,
    }

    impl RecordingHandle for FakeRecording {
        fn watchdog_snapshot(&self) -> WatchdogSnapshot {
            self.snapshot.clone()
        }

        fn stop(self: Box<Self>) -> AppResult<PathBuf> {
            self.stop_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.wav_path.clone())
        }
    }

    fn recv_output(rx: &Receiver<ControllerOutput>) -> ControllerOutput {
        rx.recv_timeout(Duration::from_secs(2))
            .expect("timed out waiting for controller output")
    }

    fn sample_transcript_result() -> TranscriptResult {
        TranscriptResult {
            run_id: "run-1".to_owned(),
            backend: BackendKind::WhisperCpp,
            transcript: "hello world".to_owned(),
            language: Some("en".to_owned()),
            warnings: Vec::new(),
            finished_at_rfc3339: "2026-02-25T00:00:02Z".to_owned(),
        }
    }

    fn sample_doctor_report() -> DoctorReport {
        DoctorReport {
            generated_at_rfc3339: "2026-02-25T00:00:00Z".to_owned(),
            state: DoctorState::Ready,
            checks: Vec::new(),
        }
    }

    fn spawn_stub_worker(
        event_tx: Sender<ControllerEvent>,
        completion_rx: Receiver<Result<TranscriptResult, String>>,
        exited: Arc<AtomicBool>,
    ) -> (Sender<WorkerMessage>, thread::JoinHandle<()>) {
        let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerMessage>();
        let join = thread::spawn(move || {
            while let Ok(message) = worker_rx.recv() {
                match message {
                    WorkerMessage::Transcribe {
                        wav_path,
                        db_path: _,
                        config: _,
                    } => {
                        let completion = completion_rx
                            .recv()
                            .unwrap_or_else(|_| Err("completion channel closed".to_owned()));
                        if event_tx
                            .send(ControllerEvent::TranscriptionFinished {
                                wav_path,
                                result: completion,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    WorkerMessage::Shutdown => break,
                }
            }
            exited.store(true, Ordering::SeqCst);
        });
        (worker_tx, join)
    }

    fn sample_context(root: &std::path::Path) -> ControllerContext {
        let mut config = AppConfig::default();
        config.history.db_path = Some(root.join("history.sqlite3"));
        config.output.mode = OutputMode::Disabled;
        config.audio.retain_audio = true;

        ControllerContext {
            config,
            paths: AppPaths {
                config_dir: root.join("config"),
                data_dir: root.join("data"),
                cache_dir: root.join("cache"),
                logs_dir: root.join("cache/logs"),
                state_dir: root.join("cache/fw-state"),
                config_file: root.join("config/config.toml"),
                history_db: root.join("data/history.sqlite3"),
                autostart_file: root.join("autostart/quedo-daemon.desktop"),
            },
        }
    }

    #[test]
    fn send_helpers_emit_expected_outputs() {
        let (tx, rx) = crossbeam_channel::unbounded::<ControllerOutput>();
        send_state(&tx, &ControllerState::Idle).expect("state");
        send_notification(&tx, "hello").expect("notify");

        assert!(matches!(
            rx.recv().expect("recv"),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));
        assert!(matches!(
            rx.recv().expect("recv"),
            ControllerOutput::Notification(message) if message == "hello"
        ));
    }

    #[test]
    fn spawn_next_job_sends_transcribe_message() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let requested = PathBuf::from("/tmp/a.wav");
        let mut queue = SingleFlightQueue::new(1);
        queue.enqueue(requested.clone()).expect("enqueue");
        let (worker_tx, worker_rx) = crossbeam_channel::unbounded::<WorkerMessage>();
        let (output_tx, _output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();

        spawn_next_job(
            &context,
            &mut queue,
            &worker_tx,
            &output_tx,
            requested.as_path(),
        )
        .expect("spawn");

        match worker_rx.recv().expect("message") {
            WorkerMessage::Transcribe {
                wav_path,
                db_path,
                config: _,
            } => {
                assert_eq!(wav_path, requested);
                assert_eq!(db_path, context.config.history.db_path.expect("db path"));
            }
            WorkerMessage::Shutdown => panic!("unexpected shutdown"),
        }
    }

    #[test]
    fn spawn_next_job_detects_queue_mismatch() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let expected = PathBuf::from("/tmp/expected.wav");
        let queued = PathBuf::from("/tmp/other.wav");
        let mut queue = SingleFlightQueue::new(1);
        queue.enqueue(queued).expect("enqueue");
        let (worker_tx, _worker_rx) = crossbeam_channel::unbounded::<WorkerMessage>();
        let (output_tx, _output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();

        let error = spawn_next_job(
            &context,
            &mut queue,
            &worker_tx,
            &output_tx,
            expected.as_path(),
        )
        .expect_err("mismatch");
        assert!(error.to_string().contains("queue scheduling mismatch"));
    }

    #[test]
    fn controller_state_machine_transitions_idle_recording_processing_idle() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let wav_path = temp.path().join("capture.wav");
        let stop_count = Arc::new(AtomicUsize::new(0));
        let stop_count_for_recording = stop_count.clone();
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<ControllerEvent>();
        let (output_tx, output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();
        let (completion_tx, completion_rx) =
            crossbeam_channel::unbounded::<Result<TranscriptResult, String>>();
        let worker_exited = Arc::new(AtomicBool::new(false));
        let (worker_tx, worker_join) =
            spawn_stub_worker(event_tx.clone(), completion_rx, worker_exited.clone());

        let controller = thread::spawn(move || {
            run_controller_loop_with(
                context,
                event_rx,
                output_tx,
                move |_output_dir, _watchdog| {
                    Ok(Box::new(FakeRecording {
                        wav_path: wav_path.clone(),
                        snapshot: WatchdogSnapshot {
                            armed: true,
                            stalled: false,
                            first_frame_seen: true,
                        },
                        stop_count: stop_count_for_recording.clone(),
                    }) as Box<dyn RecordingHandle>)
                },
                |_paths, _config| sample_doctor_report(),
                |_text| Ok(()),
                WorkerHandles {
                    tx: worker_tx,
                    join: worker_join,
                },
            )
        });

        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));

        event_tx
            .send(ControllerEvent::Toggle)
            .expect("send toggle start");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Recording)
        ));
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::Notification(message) if message == "Recording started"
        ));

        event_tx
            .send(ControllerEvent::Toggle)
            .expect("send toggle stop");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Processing)
        ));

        completion_tx
            .send(Ok(sample_transcript_result()))
            .expect("send completion");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::TranscriptReady(result) if result.run_id == "run-1"
        ));
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::Notification(message) if message == "Transcription complete"
        ));

        event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
        assert!(matches!(recv_output(&output_rx), ControllerOutput::Stopped));

        controller
            .join()
            .expect("join controller")
            .expect("controller result");
        assert_eq!(stop_count.load(Ordering::SeqCst), 1);
        assert!(worker_exited.load(Ordering::SeqCst));
    }

    #[test]
    fn controller_rejects_toggle_during_processing() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let wav_path = temp.path().join("capture.wav");
        let stop_count = Arc::new(AtomicUsize::new(0));
        let stop_count_for_recording = stop_count.clone();
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<ControllerEvent>();
        let (output_tx, output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();
        let (completion_tx, completion_rx) =
            crossbeam_channel::unbounded::<Result<TranscriptResult, String>>();
        let worker_exited = Arc::new(AtomicBool::new(false));
        let (worker_tx, worker_join) =
            spawn_stub_worker(event_tx.clone(), completion_rx, worker_exited.clone());

        let controller = thread::spawn(move || {
            run_controller_loop_with(
                context,
                event_rx,
                output_tx,
                move |_output_dir, _watchdog| {
                    Ok(Box::new(FakeRecording {
                        wav_path: wav_path.clone(),
                        snapshot: WatchdogSnapshot {
                            armed: true,
                            stalled: false,
                            first_frame_seen: true,
                        },
                        stop_count: stop_count_for_recording.clone(),
                    }) as Box<dyn RecordingHandle>)
                },
                |_paths, _config| sample_doctor_report(),
                |_text| Ok(()),
                WorkerHandles {
                    tx: worker_tx,
                    join: worker_join,
                },
            )
        });

        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));

        event_tx.send(ControllerEvent::Toggle).expect("start");
        let _ = recv_output(&output_rx);
        let _ = recv_output(&output_rx);

        event_tx.send(ControllerEvent::Toggle).expect("stop");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Processing)
        ));

        event_tx
            .send(ControllerEvent::Toggle)
            .expect("toggle while processing");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::Notification(message)
                if message == "Transcription already in progress; finishing current job."
        ));

        completion_tx
            .send(Ok(sample_transcript_result()))
            .expect("send completion");
        let _ = recv_output(&output_rx);
        let _ = recv_output(&output_rx);
        let _ = recv_output(&output_rx);

        event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
        assert!(matches!(recv_output(&output_rx), ControllerOutput::Stopped));

        controller
            .join()
            .expect("join controller")
            .expect("controller result");
        assert_eq!(stop_count.load(Ordering::SeqCst), 1);
        assert!(worker_exited.load(Ordering::SeqCst));
    }

    #[test]
    fn controller_enters_degraded_then_recovers() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let wav_path = temp.path().join("capture.wav");
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_start = attempts.clone();
        let stop_count = Arc::new(AtomicUsize::new(0));
        let stop_count_for_recording = stop_count.clone();
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<ControllerEvent>();
        let (output_tx, output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();
        let (completion_tx, completion_rx) =
            crossbeam_channel::unbounded::<Result<TranscriptResult, String>>();
        let worker_exited = Arc::new(AtomicBool::new(false));
        let (worker_tx, worker_join) =
            spawn_stub_worker(event_tx.clone(), completion_rx, worker_exited.clone());

        let controller = thread::spawn(move || {
            run_controller_loop_with(
                context,
                event_rx,
                output_tx,
                move |_output_dir, _watchdog| {
                    let attempt = attempts_for_start.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Err(AppError::Capture("microphone unavailable".to_owned()))
                    } else {
                        Ok(Box::new(FakeRecording {
                            wav_path: wav_path.clone(),
                            snapshot: WatchdogSnapshot {
                                armed: true,
                                stalled: false,
                                first_frame_seen: true,
                            },
                            stop_count: stop_count_for_recording.clone(),
                        }) as Box<dyn RecordingHandle>)
                    }
                },
                |_paths, _config| sample_doctor_report(),
                |_text| Ok(()),
                WorkerHandles {
                    tx: worker_tx,
                    join: worker_join,
                },
            )
        });

        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));

        event_tx
            .send(ControllerEvent::Toggle)
            .expect("first toggle");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Degraded(reason))
                if reason.contains("recording start failed")
        ));
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::Notification(message)
                if message.contains("recording start failed")
        ));

        event_tx
            .send(ControllerEvent::Toggle)
            .expect("recover toggle");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Recording)
        ));
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::Notification(message) if message == "Recording started"
        ));

        event_tx.send(ControllerEvent::Toggle).expect("stop");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Processing)
        ));

        completion_tx
            .send(Ok(sample_transcript_result()))
            .expect("completion");
        let _ = recv_output(&output_rx);
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));
        let _ = recv_output(&output_rx);

        event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
        assert!(matches!(recv_output(&output_rx), ControllerOutput::Stopped));

        controller
            .join()
            .expect("join controller")
            .expect("controller result");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(stop_count.load(Ordering::SeqCst), 1);
        assert!(worker_exited.load(Ordering::SeqCst));
    }

    #[test]
    fn controller_shutdown_drains_worker_and_active_recording() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let context = sample_context(temp.path());
        let wav_path = temp.path().join("capture.wav");
        let stop_count = Arc::new(AtomicUsize::new(0));
        let stop_count_for_recording = stop_count.clone();
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<ControllerEvent>();
        let (output_tx, output_rx) = crossbeam_channel::unbounded::<ControllerOutput>();
        let (_completion_tx, completion_rx) =
            crossbeam_channel::unbounded::<Result<TranscriptResult, String>>();
        let worker_exited = Arc::new(AtomicBool::new(false));
        let (worker_tx, worker_join) =
            spawn_stub_worker(event_tx.clone(), completion_rx, worker_exited.clone());
        let doctor_calls = Arc::new(Mutex::new(VecDeque::new()));
        let doctor_calls_for_runner = doctor_calls.clone();

        let controller = thread::spawn(move || {
            run_controller_loop_with(
                context,
                event_rx,
                output_tx,
                move |_output_dir, _watchdog| {
                    Ok(Box::new(FakeRecording {
                        wav_path: wav_path.clone(),
                        snapshot: WatchdogSnapshot {
                            armed: true,
                            stalled: false,
                            first_frame_seen: true,
                        },
                        stop_count: stop_count_for_recording.clone(),
                    }) as Box<dyn RecordingHandle>)
                },
                move |_paths, _config| {
                    doctor_calls_for_runner
                        .lock()
                        .expect("lock doctor calls")
                        .push_back("called");
                    sample_doctor_report()
                },
                |_text| Ok(()),
                WorkerHandles {
                    tx: worker_tx,
                    join: worker_join,
                },
            )
        });

        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Idle)
        ));

        event_tx.send(ControllerEvent::Toggle).expect("start");
        assert!(matches!(
            recv_output(&output_rx),
            ControllerOutput::StateChanged(ControllerState::Recording)
        ));
        let _ = recv_output(&output_rx);

        event_tx.send(ControllerEvent::Shutdown).expect("shutdown");
        assert!(matches!(recv_output(&output_rx), ControllerOutput::Stopped));

        controller
            .join()
            .expect("join controller")
            .expect("controller result");

        assert_eq!(stop_count.load(Ordering::SeqCst), 1);
        assert!(worker_exited.load(Ordering::SeqCst));
        assert!(
            doctor_calls.lock().expect("lock doctor calls").is_empty(),
            "doctor runner should not be called in shutdown drain test"
        );
    }
}

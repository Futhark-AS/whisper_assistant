use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::bootstrap::{bootstrap_env, AppPaths};
use crate::capture::devices::list_input_devices;
use crate::config::AppConfig;
use crate::controller::events::{ControllerEvent, ControllerOutput};
use crate::controller::{run_controller_loop, ControllerContext};
use crate::error::{AppError, AppResult};
use crate::history::HistoryStore;
use crate::runtime::topology::RuntimeTopology;
use crate::ui::{Notifier, UiFrontend};

pub fn run_app(config: AppConfig, paths: AppPaths) -> AppResult<()> {
    paths.ensure_dirs()?;
    bootstrap_env(&paths)?;

    let topology = RuntimeTopology::new();
    let controller_context = ControllerContext {
        config: config.clone(),
        paths: paths.clone(),
    };

    let controller_event_tx = topology.controller_event_tx.clone();
    let controller_output_tx = topology.controller_output_tx.clone();

    let controller_join = thread::Builder::new()
        .name("quedo-controller".to_owned())
        .spawn(move || {
            let _ = run_controller_loop(
                controller_context,
                topology.controller_event_rx,
                controller_event_tx,
                controller_output_tx,
            );
        })
        .map_err(|error| AppError::Controller(format!("failed to spawn controller: {error}")))?;

    let notifier = Notifier::new(config.output.enable_notifications);
    let ui = UiFrontend::new(&config.hotkey.binding)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_flag.store(true, Ordering::SeqCst);
    })
    .map_err(|error| AppError::Controller(format!("failed to register ctrl-c handler: {error}")))?;

    #[cfg(not(target_os = "macos"))]
    spawn_stdin_command_thread(topology.controller_event_tx.clone())?;

    let mut last_tick = Instant::now();
    let mut stopping = false;

    loop {
        if !stopping && last_tick.elapsed() >= Duration::from_millis(150) {
            let _ = topology.controller_event_tx.send(ControllerEvent::Tick);
            last_tick = Instant::now();
        }

        for event in ui.drain_events() {
            if matches!(event, ControllerEvent::Shutdown) {
                stopping = true;
            }
            let _ = topology.controller_event_tx.send(event);
        }

        if !stopping && shutdown.load(Ordering::SeqCst) {
            stopping = true;
            let _ = topology.controller_event_tx.send(ControllerEvent::Shutdown);
        }

        while let Ok(output) = topology.controller_output_rx.try_recv() {
            match output {
                ControllerOutput::StateChanged(state) => {
                    ui.set_state(&state)?;
                    if let crate::controller::state::ControllerState::Degraded(reason) = &state {
                        let _ = notifier.notify("Quedo Degraded", reason);
                    }
                }
                ControllerOutput::Notification(message) => {
                    tracing::info!("{message}");
                    let _ = notifier.notify("Quedo", &message);
                }
                ControllerOutput::DoctorReport(report) => {
                    tracing::info!("doctor report emitted");
                    println!("{}", report.render_text());
                }
                ControllerOutput::TranscriptReady(result) => {
                    tracing::info!(run_id = %result.run_id, "transcript copied to clipboard");
                    let _ = notifier.notify("Quedo", "Transcript copied to clipboard");
                }
                ControllerOutput::Stopped => {
                    controller_join.join().map_err(|_| {
                        AppError::Controller("controller thread panicked".to_owned())
                    })?;
                    return Ok(());
                }
            }
        }

        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(not(target_os = "macos"))]
fn spawn_stdin_command_thread(
    event_tx: crossbeam_channel::Sender<ControllerEvent>,
) -> AppResult<()> {
    thread::Builder::new()
        .name("quedo-stdin-events".to_owned())
        .spawn(move || {
            use std::io::{self, BufRead};

            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                let command = line.trim().to_ascii_lowercase();
                let event = match command.as_str() {
                    "toggle" => Some(ControllerEvent::Toggle),
                    "doctor" => Some(ControllerEvent::RunDoctor),
                    "quit" | "exit" => Some(ControllerEvent::Shutdown),
                    _ => None,
                };

                if let Some(event) = event {
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| {
            AppError::Controller(format!("failed to spawn stdin event thread: {error}"))
        })
}

pub fn install_autostart(paths: &AppPaths) -> AppResult<PathBuf> {
    paths.ensure_dirs()?;

    let executable = std::env::current_exe().map_err(|error| {
        AppError::Install(format!("unable to resolve current executable: {error}"))
    })?;

    if cfg!(target_os = "macos") {
        let plist = format!(
            r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
  <key>Label</key>
  <string>io.quedo.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
</dict>
</plist>
"#,
            executable.display()
        );
        std::fs::write(&paths.autostart_file, plist)?;
    } else {
        let desktop_entry = format!(
            "[Desktop Entry]\nType=Application\nName=Quedo Daemon\nExec={} run\nX-GNOME-Autostart-enabled=true\n",
            executable.display()
        );
        std::fs::write(&paths.autostart_file, desktop_entry)?;
    }

    Ok(paths.autostart_file.clone())
}

pub fn status_report(config: &AppConfig, paths: &AppPaths) -> AppResult<String> {
    let db_path = config
        .history
        .db_path
        .clone()
        .unwrap_or_else(|| paths.history_db.clone());
    let history = HistoryStore::new(db_path.clone());
    let recent = history.list_recent_runs(5)?;
    let latest = history.latest_run()?;
    let recording_capability = match list_input_devices() {
        Ok(devices) if !devices.is_empty() => "available".to_owned(),
        Ok(_) => "unavailable (no input devices)".to_owned(),
        Err(error) => format!("unavailable ({error})"),
    };

    let mut output = String::new();
    output.push_str("Quedo daemon status\n");
    output.push_str(&format!("  config: {}\n", paths.config_file.display()));
    output.push_str(&format!("  history_db: {}\n", db_path.display()));
    output.push_str(&format!(
        "  franken_state_dir: {}\n",
        paths.state_dir.display()
    ));
    output.push_str(&format!("  recording_backend: {recording_capability}\n"));
    output.push_str(&format!("  recent_runs: {}\n", recent.len()));

    if let Some(run) = latest {
        output.push_str(&format!(
            "  last_run: {} backend={:?} finished={}\n",
            run.run_id, run.backend, run.finished_at_rfc3339
        ));
    }

    Ok(output)
}

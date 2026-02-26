use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::TryRecvError;

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

    let RuntimeTopology {
        controller_event_tx,
        controller_event_rx,
        controller_output_tx,
        controller_output_rx,
    } = RuntimeTopology::new();
    let controller_context = ControllerContext {
        config: config.clone(),
        paths: paths.clone(),
    };
    let controller_event_tx_for_loop = controller_event_tx.clone();
    let (controller_result_tx, controller_result_rx) = crossbeam_channel::bounded(1);

    let mut controller_join = Some(
        thread::Builder::new()
            .name("quedo-controller".to_owned())
            .spawn(move || {
                let result = run_controller_loop(
                    controller_context,
                    controller_event_rx,
                    controller_event_tx_for_loop,
                    controller_output_tx,
                );
                let _ = controller_result_tx.send(result);
            })
            .map_err(|error| {
                AppError::Controller(format!("failed to spawn controller: {error}"))
            })?,
    );

    let notifier = Notifier::new(config.output.enable_notifications);
    let ui = UiFrontend::new(&config.hotkey.binding)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_flag.store(true, Ordering::SeqCst);
    })
    .map_err(|error| AppError::Controller(format!("failed to register ctrl-c handler: {error}")))?;

    #[cfg(not(target_os = "macos"))]
    spawn_stdin_command_thread(controller_event_tx.clone())?;

    let mut last_tick = Instant::now();
    let mut stopping = false;

    loop {
        if !stopping && last_tick.elapsed() >= Duration::from_millis(150) {
            let _ = controller_event_tx.send(ControllerEvent::Tick);
            last_tick = Instant::now();
        }

        for event in ui.drain_events() {
            if matches!(event, ControllerEvent::Shutdown) {
                stopping = true;
            }
            let _ = controller_event_tx.send(event);
        }

        if !stopping && shutdown.load(Ordering::SeqCst) {
            stopping = true;
            let _ = controller_event_tx.send(ControllerEvent::Shutdown);
        }

        loop {
            match controller_output_rx.try_recv() {
                Ok(output) => match output {
                    ControllerOutput::StateChanged(state) => {
                        ui.set_state(&state)?;
                        match &state {
                            crate::controller::state::ControllerState::Degraded(reason) => {
                                let _ = notifier.notify("Quedo Degraded", reason);
                            }
                            crate::controller::state::ControllerState::Unavailable(reason) => {
                                let _ = notifier.notify("Quedo Unavailable", reason);
                            }
                            _ => {}
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
                        let join_result = controller_join
                            .take()
                            .expect("controller join handle missing")
                            .join()
                            .map_err(|_| {
                                AppError::Controller("controller thread panicked".to_owned())
                            });
                        let loop_result = controller_result_rx.recv().map_err(|_| {
                            AppError::Controller(
                                "controller result channel closed before completion".to_owned(),
                            )
                        })?;
                        join_result?;
                        loop_result?;
                        return Ok(());
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let join_result = controller_join
                        .take()
                        .expect("controller join handle missing")
                        .join()
                        .map_err(|_| AppError::Controller("controller thread panicked".to_owned()));
                    let loop_result = controller_result_rx.try_recv().ok().unwrap_or_else(|| {
                        Err(AppError::Controller(
                            "controller output channel disconnected".to_owned(),
                        ))
                    });
                    join_result?;
                    return loop_result;
                }
            }
        }

        match controller_result_rx.try_recv() {
            Ok(loop_result) => {
                controller_join
                    .take()
                    .expect("controller join handle missing")
                    .join()
                    .map_err(|_| AppError::Controller("controller thread panicked".to_owned()))?;
                return match loop_result {
                    Ok(()) => Err(AppError::Controller(
                        "controller loop exited without stop signal".to_owned(),
                    )),
                    Err(error) => Err(error),
                };
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(AppError::Controller(
                    "controller result channel disconnected".to_owned(),
                ))
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
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
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

#[cfg(test)]
mod tests {
    use super::{install_autostart, status_report};
    use crate::bootstrap::paths::AppPaths;
    use crate::config::schema::AppConfig;
    use rusqlite::Connection;

    fn make_paths(root: &std::path::Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            logs_dir: root.join("cache/logs"),
            state_dir: root.join("cache/fw-state"),
            config_file: root.join("config/config.toml"),
            history_db: root.join("data/history.sqlite3"),
            autostart_file: if cfg!(target_os = "macos") {
                root.join("LaunchAgents/io.quedo.daemon.plist")
            } else {
                root.join("autostart/quedo-daemon.desktop")
            },
        }
    }

    #[test]
    fn install_autostart_writes_expected_template() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let paths = make_paths(temp.path());
        let installed = install_autostart(&paths).expect("install");
        assert_eq!(installed, paths.autostart_file);
        let text = std::fs::read_to_string(installed).expect("read");
        let executable = std::env::current_exe().expect("current exe");
        if cfg!(target_os = "macos") {
            assert!(text.contains("<plist"));
            assert!(text.contains("<key>ProgramArguments</key>"));
            assert!(text.contains(&format!("<string>{}</string>", executable.display())));
            assert!(text.contains("<string>run</string>"));
            assert!(
                !text.contains("\\\""),
                "plist should not contain literal escaped quotes"
            );
            let parsed = roxmltree::Document::parse(&text).expect("valid plist xml");
            assert_eq!(parsed.root_element().tag_name().name(), "plist");
        } else {
            assert!(text.contains("[Desktop Entry]"));
            assert!(text.contains("Type=Application"));
            assert!(text.contains("Name=Quedo Daemon"));
            assert!(text.contains("X-GNOME-Autostart-enabled=true"));
            let expected_exec = format!("Exec={} run", executable.display());
            assert!(
                text.lines().any(|line| line == expected_exec),
                "autostart entry missing exact Exec line `{expected_exec}`:\n{text}"
            );
        }
    }

    #[test]
    fn status_report_contains_paths_and_history_summary() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let paths = make_paths(temp.path());
        paths.ensure_dirs().expect("dirs");

        let conn = Connection::open(&paths.history_db).expect("open db");
        conn.execute_batch(
            "CREATE TABLE runs (
                id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                backend TEXT NOT NULL,
                transcript TEXT NOT NULL
            );",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "run-1",
                "2026-02-25T00:00:00Z",
                "2026-02-25T00:00:01Z",
                "whisper_cpp",
                "hello world",
            ),
        )
        .expect("insert");

        let config = AppConfig::default();
        let report = status_report(&config, &paths).expect("report");
        assert!(report.contains("Quedo daemon status"));
        assert!(report.contains("config:"));
        assert!(report.contains("history_db:"));
        assert!(report.contains("franken_state_dir:"));
        assert!(report.contains("recording_backend:"));
        assert!(report.contains("recent_runs: 1"));
        assert!(report.contains("last_run: run-1"));
    }
}

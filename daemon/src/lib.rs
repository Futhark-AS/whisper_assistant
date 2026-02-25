pub mod bootstrap;
pub mod capture;
pub mod cli;
pub mod config;
pub mod controller;
pub mod doctor;
pub mod error;
pub mod history;
pub mod output;
pub mod runtime;
#[cfg(test)]
mod test_support;
pub mod transcription;
pub mod ui;

use clap::Parser;

use crate::bootstrap::AppPaths;
use crate::cli::{Cli, Command};
use crate::config::load_config;
use crate::doctor::run_doctor;
use crate::error::AppResult;
use crate::runtime::{install_autostart, run_app, status_report};

trait CommandExecutor {
    fn run(&self, config: crate::config::AppConfig, paths: AppPaths) -> AppResult<()>;
    fn doctor(
        &self,
        paths: &AppPaths,
        config: &crate::config::AppConfig,
        json: bool,
    ) -> AppResult<()>;
    fn install(&self, paths: &AppPaths, config: &crate::config::AppConfig) -> AppResult<()>;
    fn status(&self, paths: &AppPaths, config: &crate::config::AppConfig) -> AppResult<()>;
}

struct DefaultCommandExecutor;

impl CommandExecutor for DefaultCommandExecutor {
    fn run(&self, config: crate::config::AppConfig, paths: AppPaths) -> AppResult<()> {
        run_app(config, paths)
    }

    fn doctor(
        &self,
        paths: &AppPaths,
        config: &crate::config::AppConfig,
        json: bool,
    ) -> AppResult<()> {
        let report = run_doctor(paths, config);
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", report.render_text());
        }
        Ok(())
    }

    fn install(&self, paths: &AppPaths, config: &crate::config::AppConfig) -> AppResult<()> {
        let installed_path = install_autostart(paths)?;
        println!("Installed autostart entry: {}", installed_path.display());

        let report = run_doctor(paths, config);
        println!("{}", report.render_text());
        Ok(())
    }

    fn status(&self, paths: &AppPaths, config: &crate::config::AppConfig) -> AppResult<()> {
        let report = status_report(config, paths)?;
        println!("{report}");
        Ok(())
    }
}

fn execute_command<E: CommandExecutor>(
    command: Command,
    paths: AppPaths,
    config: crate::config::AppConfig,
    executor: &E,
) -> AppResult<()> {
    match command {
        Command::Run => executor.run(config, paths),
        Command::Doctor { json } => executor.doctor(&paths, &config, json),
        Command::Install => executor.install(&paths, &config),
        Command::Status => executor.status(&paths, &config),
    }
}

pub fn run() -> AppResult<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    let cli = Cli::parse();

    let paths = AppPaths::resolve()?;
    paths.ensure_dirs()?;

    let config = load_config(&paths, &cli.to_overrides())?;

    execute_command(cli.command, paths, config, &DefaultCommandExecutor)
}

#[cfg(test)]
mod tests {
    use super::{execute_command, CommandExecutor};
    use crate::bootstrap::paths::AppPaths;
    use crate::cli::Command;
    use crate::config::schema::AppConfig;
    use crate::error::AppResult;
    use std::sync::Mutex;

    #[derive(Default)]
    struct SpyExecutor {
        calls: Mutex<Vec<String>>,
    }

    impl CommandExecutor for SpyExecutor {
        fn run(&self, _config: AppConfig, _paths: AppPaths) -> AppResult<()> {
            self.calls
                .lock()
                .expect("lock calls")
                .push("run".to_owned());
            Ok(())
        }

        fn doctor(&self, _paths: &AppPaths, _config: &AppConfig, json: bool) -> AppResult<()> {
            self.calls
                .lock()
                .expect("lock calls")
                .push(format!("doctor:{json}"));
            Ok(())
        }

        fn install(&self, _paths: &AppPaths, _config: &AppConfig) -> AppResult<()> {
            self.calls
                .lock()
                .expect("lock calls")
                .push("install".to_owned());
            Ok(())
        }

        fn status(&self, _paths: &AppPaths, _config: &AppConfig) -> AppResult<()> {
            self.calls
                .lock()
                .expect("lock calls")
                .push("status".to_owned());
            Ok(())
        }
    }

    fn sample_paths(root: &std::path::Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            logs_dir: root.join("cache/logs"),
            state_dir: root.join("cache/fw-state"),
            config_file: root.join("config/config.toml"),
            history_db: root.join("data/history.sqlite3"),
            autostart_file: root.join("autostart/quedo-daemon.desktop"),
        }
    }

    #[test]
    fn command_dispatch_routes_run_doctor_install_and_status() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let paths = sample_paths(temp.path());
        let config = AppConfig::default();
        let executor = SpyExecutor::default();

        execute_command(Command::Run, paths.clone(), config.clone(), &executor).expect("run");
        execute_command(
            Command::Doctor { json: true },
            paths.clone(),
            config.clone(),
            &executor,
        )
        .expect("doctor");
        execute_command(Command::Install, paths.clone(), config.clone(), &executor)
            .expect("install");
        execute_command(Command::Status, paths, config, &executor).expect("status");

        assert_eq!(
            executor.calls.lock().expect("lock calls").as_slice(),
            ["run", "doctor:true", "install", "status"]
        );
    }

    #[test]
    fn module_re_exports_are_reachable() {
        let _bootstrap: fn(
            &crate::bootstrap::AppPaths,
        ) -> crate::error::AppResult<std::path::PathBuf> = crate::bootstrap::bootstrap_env;
        let _config_load: fn(
            &crate::bootstrap::AppPaths,
            &crate::config::CliOverrides,
        ) -> crate::error::AppResult<crate::config::AppConfig> = crate::config::load_config;
        let _runtime_install: fn(
            &crate::bootstrap::AppPaths,
        ) -> crate::error::AppResult<std::path::PathBuf> = crate::runtime::install_autostart;
        let _runtime_status: fn(
            &crate::config::AppConfig,
            &crate::bootstrap::AppPaths,
        ) -> crate::error::AppResult<String> = crate::runtime::status_report;
        let _capture_ctor: fn(Option<String>) -> crate::capture::MicrophoneCapture =
            crate::capture::MicrophoneCapture::new;
        let _transcribe_job: fn(
            &crate::transcription::FrankenEngine,
            std::path::PathBuf,
            std::path::PathBuf,
            &crate::config::TranscriptionConfig,
        )
            -> crate::error::AppResult<crate::transcription::TranscriptResult> =
            crate::transcription::run_transcription_job;
        let _doctor: fn(
            &crate::bootstrap::AppPaths,
            &crate::config::AppConfig,
        ) -> crate::doctor::DoctorReport = crate::doctor::run_doctor;
        let _history_ctor: fn(std::path::PathBuf) -> crate::history::HistoryStore =
            crate::history::HistoryStore::new;
        let _clipboard_write: fn(&str) -> crate::error::AppResult<()> =
            crate::output::ClipboardOutput::write_text;
        let _notifier_ctor: fn(bool) -> crate::ui::Notifier = crate::ui::Notifier::new;
    }
}

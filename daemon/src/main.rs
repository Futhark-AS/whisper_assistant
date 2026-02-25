mod bootstrap;
mod capture;
mod cli;
mod config;
mod controller;
mod doctor;
mod error;
mod history;
mod output;
mod runtime;
#[cfg(test)]
mod test_support;
mod transcription;
mod ui;

use clap::Parser;

use crate::bootstrap::AppPaths;
use crate::cli::{Cli, Command};
use crate::config::load_config;
use crate::doctor::run_doctor;
use crate::error::AppResult;
use crate::runtime::{install_autostart, run_app, status_report};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
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

    match cli.command {
        Command::Run => run_app(config, paths),
        Command::Doctor { json } => {
            let report = run_doctor(&paths, &config);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.render_text());
            }
            Ok(())
        }
        Command::Install => {
            let installed_path = install_autostart(&paths)?;
            println!("Installed autostart entry: {}", installed_path.display());

            let report = run_doctor(&paths, &config);
            println!("{}", report.render_text());

            Ok(())
        }
        Command::Status => {
            let report = status_report(&config, &paths)?;
            println!("{report}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_re_exports_are_reachable() {
        let _bootstrap: fn(&crate::bootstrap::AppPaths) -> crate::error::AppResult<std::path::PathBuf> =
            crate::bootstrap::bootstrap_env;
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
        ) -> crate::error::AppResult<crate::transcription::TranscriptResult> =
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

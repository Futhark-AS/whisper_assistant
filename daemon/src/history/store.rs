use std::path::PathBuf;

use franken_whisper::BackendKind;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::history::models::RunSummary;

pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn list_recent_runs(&self, limit: usize) -> AppResult<Vec<RunSummary>> {
        if !self.db_path.exists() {
            return Ok(Vec::new());
        }

        let connection = Connection::open(&self.db_path)?;
        let limit = if limit == 0 { i64::MAX } else { limit as i64 };
        let sql = "SELECT id, started_at, finished_at, backend, transcript \
                   FROM runs ORDER BY started_at DESC LIMIT ?1";

        let mut statement = match connection.prepare(sql) {
            Ok(statement) => statement,
            Err(error) => return handle_missing_schema(error),
        };

        let rows = statement.query_map([limit], |row| {
            let run_id: String = row.get(0)?;
            let started_at_rfc3339: String = row.get(1)?;
            let finished_at_rfc3339: String = row.get(2)?;
            let backend_raw: String = row.get(3)?;
            let transcript: String = row.get(4)?;

            Ok(RunSummary {
                run_id,
                started_at_rfc3339,
                finished_at_rfc3339,
                backend: parse_backend(&backend_raw),
                transcript_preview: transcript.chars().take(140).collect(),
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn latest_run(&self) -> AppResult<Option<RunSummary>> {
        let mut runs = self.list_recent_runs(1)?;
        Ok(runs.pop())
    }
}

fn parse_backend(raw: &str) -> BackendKind {
    match raw {
        "auto" => BackendKind::Auto,
        "whisper_cpp" => BackendKind::WhisperCpp,
        "insanely_fast" => BackendKind::InsanelyFast,
        "whisper_diarization" => BackendKind::WhisperDiarization,
        _ => BackendKind::Auto,
    }
}

fn handle_missing_schema(error: rusqlite::Error) -> AppResult<Vec<RunSummary>> {
    match &error {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("no such table") || message.contains("no such column") =>
        {
            Ok(Vec::new())
        }
        _ => Err(AppError::Sqlite(error)),
    }
}

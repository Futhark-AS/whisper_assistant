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

#[cfg(test)]
mod tests {
    use super::{parse_backend, HistoryStore};
    use franken_whisper::BackendKind;
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn build_store(path: PathBuf) -> HistoryStore {
        HistoryStore::new(path)
    }

    #[test]
    fn backend_parser_maps_known_values_and_defaults_unknown() {
        assert_eq!(parse_backend("auto"), BackendKind::Auto);
        assert_eq!(parse_backend("whisper_cpp"), BackendKind::WhisperCpp);
        assert_eq!(parse_backend("insanely_fast"), BackendKind::InsanelyFast);
        assert_eq!(
            parse_backend("whisper_diarization"),
            BackendKind::WhisperDiarization
        );
        assert_eq!(parse_backend("unknown"), BackendKind::Auto);
    }

    #[test]
    fn returns_empty_when_db_missing() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let db = temp.path().join("missing.sqlite3");
        let store = build_store(db);
        let runs = store.list_recent_runs(10).expect("list");
        assert!(runs.is_empty());
        assert!(store.latest_run().expect("latest").is_none());
    }

    #[test]
    fn handles_missing_schema_gracefully() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let db = temp.path().join("history.sqlite3");
        let _ = Connection::open(&db).expect("create db");
        let store = build_store(db);
        let runs = store.list_recent_runs(10).expect("list");
        assert!(runs.is_empty());
    }

    #[test]
    fn maps_rows_and_truncates_preview_to_140_chars() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let db = temp.path().join("history.sqlite3");
        let conn = Connection::open(&db).expect("open");
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
        let long = "x".repeat(200);
        conn.execute(
            "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "run-1",
                "2026-02-25T00:00:00Z",
                "2026-02-25T00:00:01Z",
                "whisper_cpp",
                &long,
            ),
        )
        .expect("insert");

        let store = build_store(db);
        let runs = store.list_recent_runs(5).expect("list");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].backend, BackendKind::WhisperCpp);
        assert_eq!(runs[0].transcript_preview.len(), 140);
    }

    #[test]
    fn latest_run_returns_none_or_newest_row() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let db = temp.path().join("history.sqlite3");
        let conn = Connection::open(&db).expect("open");
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

        let store = build_store(db.clone());
        assert!(store.latest_run().expect("latest").is_none());

        conn.execute(
            "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "old",
                "2026-02-25T00:00:00Z",
                "2026-02-25T00:00:01Z",
                "auto",
                "one",
            ),
        )
        .expect("insert old");
        conn.execute(
            "INSERT INTO runs (id, started_at, finished_at, backend, transcript)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "new",
                "2026-02-25T01:00:00Z",
                "2026-02-25T01:00:01Z",
                "insanely_fast",
                "two",
            ),
        )
        .expect("insert new");

        let latest = store.latest_run().expect("latest").expect("some");
        assert_eq!(latest.run_id, "new");
        assert_eq!(latest.backend, BackendKind::InsanelyFast);
    }
}

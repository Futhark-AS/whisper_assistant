use std::path::PathBuf;

use franken_whisper::model::{BackendParams, InputSource};
use franken_whisper::TranscribeRequest;

use crate::config::TranscriptionConfig;

pub fn build_request(
    wav_path: PathBuf,
    db_path: PathBuf,
    cfg: &TranscriptionConfig,
) -> TranscribeRequest {
    TranscribeRequest {
        input: InputSource::File { path: wav_path },
        backend: cfg.backend,
        model: cfg.model_id.clone(),
        language: cfg.language.clone(),
        translate: cfg.translate,
        diarize: cfg.diarize,
        persist: true,
        db_path,
        timeout_ms: Some(cfg.timeout_ms()),
        backend_params: BackendParams {
            threads: cfg.threads,
            processors: cfg.processors,
            ..BackendParams::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::build_request;
    use crate::config::schema::TranscriptionConfig;
    use franken_whisper::model::InputSource;
    use franken_whisper::BackendKind;
    use std::path::PathBuf;

    #[test]
    fn build_request_maps_all_fields_exactly() {
        let cfg = TranscriptionConfig {
            backend: BackendKind::InsanelyFast,
            model_id: Some("m".to_owned()),
            language: Some("en".to_owned()),
            translate: true,
            diarize: true,
            timeout_seconds: 12,
            threads: Some(7),
            processors: Some(2),
        };
        let wav = PathBuf::from("/tmp/in.wav");
        let db = PathBuf::from("/tmp/history.sqlite3");
        let request = build_request(wav.clone(), db.clone(), &cfg);

        match request.input {
            InputSource::File { path } => assert_eq!(path, wav),
            _ => panic!("expected file input"),
        }
        assert_eq!(request.backend, BackendKind::InsanelyFast);
        assert_eq!(request.model.as_deref(), Some("m"));
        assert_eq!(request.language.as_deref(), Some("en"));
        assert!(request.translate);
        assert!(request.diarize);
        assert!(request.persist);
        assert_eq!(request.db_path, db);
        assert_eq!(request.timeout_ms, Some(12_000));
        assert_eq!(request.backend_params.threads, Some(7));
        assert_eq!(request.backend_params.processors, Some(2));
    }

    #[test]
    fn build_request_uses_defaults_for_optional_fields() {
        let cfg = TranscriptionConfig::default();
        let request = build_request(
            PathBuf::from("/tmp/input.wav"),
            PathBuf::from("/tmp/history.sqlite3"),
            &cfg,
        );
        assert_eq!(request.model, None);
        assert_eq!(request.language, None);
        assert!(request.persist);
        assert_eq!(request.timeout_ms, Some(45_000));
        assert_eq!(request.backend_params.threads, None);
        assert_eq!(request.backend_params.processors, None);
    }
}

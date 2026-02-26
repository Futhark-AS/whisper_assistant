use std::path::PathBuf;

fn fixture_candidates() -> [PathBuf; 2] {
    [
        PathBuf::from("/home/jorge/.local/src/whisper.cpp/samples/jfk.wav"),
        PathBuf::from("/tmp/franken_whisper/test_data/jfk.wav"),
    ]
}

#[test]
fn release_gate_assets_are_present() {
    for binary in ["ffmpeg", "ffprobe", "whisper-cli", "python3"] {
        which::which(binary)
            .unwrap_or_else(|_| panic!("release gate requires `{binary}` to be available in PATH"));
    }

    let fixture = fixture_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("required fixture missing at expected paths"));
    assert!(
        fixture.metadata().map(|m| m.len() > 0).unwrap_or(false),
        "fixture must be a non-empty file: {}",
        fixture.display()
    );

    let model = PathBuf::from("/home/jorge/.local/share/quedo/models/ggml-base.en.bin");
    assert!(
        model.is_file(),
        "release gate requires model file at {}",
        model.display()
    );
}

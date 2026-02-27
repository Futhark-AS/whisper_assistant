#!/usr/bin/env bash
set -euo pipefail

MODEL_VARIANT="${1:-large-v3-turbo}"
MODEL_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/whisper"
MODEL_PATH="$MODEL_DIR/ggml-${MODEL_VARIANT}.bin"

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required: https://brew.sh" >&2
  exit 1
fi

if ! command -v whisper-cli >/dev/null 2>&1; then
  brew install whisper-cpp
fi

mkdir -p "$MODEL_DIR"
if [[ ! -s "$MODEL_PATH" ]]; then
  curl -L --fail --retry 3 \
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL_VARIANT}.bin" \
    -o "$MODEL_PATH"
fi

SAMPLE_AUDIO="$(brew --prefix whisper-cpp)/share/whisper-cpp/jfk.wav"
SMOKE_PREFIX="/tmp/quedo-whisper-smoke"
whisper-cli -m "$MODEL_PATH" -f "$SAMPLE_AUDIO" -otxt -nt -np -of "$SMOKE_PREFIX" >/dev/null 2>&1

if [[ ! -s "${SMOKE_PREFIX}.txt" ]]; then
  echo "Smoke test failed: no transcript output generated" >&2
  exit 1
fi

echo "whisper.cpp bootstrap complete"
echo "binary: $(command -v whisper-cli)"
echo "model:  $MODEL_PATH"
echo "Set Quedo Preferences -> Providers -> whisper.cpp model to:"
echo "  $MODEL_PATH"

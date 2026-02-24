#!/bin/bash
set -e

INPUT_DIR="$HOME/h/dev/quedo/transcription-inputs"
OUTPUT_DIR="$HOME/h/dev/quedo/transcription-results"
SCRIPT_DIR="$(dirname "$0")"

for f in "$INPUT_DIR"/*; do
    name=$(basename "$f" .mov)
    echo "Processing: $name"
    uv run "$SCRIPT_DIR/diarization.py" "$f" "$OUTPUT_DIR/$name.txt"
    echo ""
done

echo "Done!"

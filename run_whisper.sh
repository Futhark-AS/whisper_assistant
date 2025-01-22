#!/bin/bash

# Path to the binary
BINARY_PATH="/Users/jorgensandhaug/github_documents/whisper_assistant/dist/whisperGPT"

# Check if the binary exists
if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: Binary not found at $BINARY_PATH"
  exit 1
fi

# Execute the binary
"$BINARY_PATH" "$@"

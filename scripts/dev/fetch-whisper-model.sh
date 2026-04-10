#!/usr/bin/env bash
set -euo pipefail

MODEL_NAME="${1:-base}"
mkdir -p models

case "$MODEL_NAME" in
  base|small|medium|large-v3)
    ;;
  *)
    echo "Unsupported model '$MODEL_NAME'. Use one of: base, small, medium, large-v3" >&2
    exit 1
    ;;
esac

TARGET_PATH="models/ggml-${MODEL_NAME}.bin"
SOURCE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL_NAME}.bin"

echo "Downloading ${MODEL_NAME} model to ${TARGET_PATH}..."
curl -L --fail --progress-bar "$SOURCE_URL" -o "$TARGET_PATH"
echo "Done: ${TARGET_PATH}"

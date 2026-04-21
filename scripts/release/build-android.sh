#!/usr/bin/env bash
set -euo pipefail

ARTIFACT="${1:-}"
MODE="${2:-}"

if [[ "$ARTIFACT" != "apk" && "$ARTIFACT" != "aab" ]]; then
  echo "Usage: ./scripts/release/build-android.sh <apk|aab> <debug|release>" >&2
  exit 1
fi

if [[ "$MODE" != "debug" && "$MODE" != "release" ]]; then
  echo "Usage: ./scripts/release/build-android.sh <apk|aab> <debug|release>" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ANDROID_DIR="$ROOT_DIR/apps/android"
ANDROID_PKG="$ANDROID_DIR/package.json"
ANDROID_TAURI_CONF="$ANDROID_DIR/src-tauri/tauri.conf.json"

if [[ ! -f "$ANDROID_PKG" || ! -f "$ANDROID_TAURI_CONF" ]]; then
  cat >&2 <<'EOF'
Android build is not configured yet in this repository.

Expected files:
- apps/android/package.json
- apps/android/src-tauri/tauri.conf.json

Current status: apps/android is scaffold-only (planned shell).
Initialize the Android Tauri app in apps/android, then re-run this command.
EOF
  exit 1
fi

BUNDLE_FLAG="--apk"
if [[ "$ARTIFACT" == "aab" ]]; then
  BUNDLE_FLAG="--aab"
fi

MODE_FLAG="--release"
if [[ "$MODE" == "debug" ]]; then
  MODE_FLAG="--debug"
fi

exec pnpm --dir "$ANDROID_DIR" exec tauri android build "$BUNDLE_FLAG" "$MODE_FLAG"

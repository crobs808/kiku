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
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
TAURI_DIR="$DESKTOP_DIR/src-tauri"
TAURI_CONF="$TAURI_DIR/tauri.conf.json"
ANDROID_GEN_DIR="$TAURI_DIR/gen/android"
ANDROID_RES_DIR="$ANDROID_GEN_DIR/app/src/main/res"
ICON_SRC_DIR="$TAURI_DIR/icons/android"

if [[ ! -f "$DESKTOP_DIR/package.json" || ! -f "$TAURI_CONF" ]]; then
  echo "Android build is not configured in apps/desktop." >&2
  exit 1
fi

ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
NDK_VERSION="${NDK_VERSION:-29.0.14206865}"
NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/$NDK_VERSION}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$NDK_HOME}"
ANDROID_NDK="${ANDROID_NDK:-$NDK_HOME}"
CMAKE_TOOLCHAIN_FILE="${CMAKE_TOOLCHAIN_FILE:-$NDK_HOME/build/cmake/android.toolchain.cmake}"
CMDLINE_TOOLS_BIN="${CMDLINE_TOOLS_BIN:-/opt/homebrew/share/android-commandlinetools/cmdline-tools/latest/bin}"
if [[ -d "$CMDLINE_TOOLS_BIN" ]]; then
  export PATH="$CMDLINE_TOOLS_BIN:$PATH"
fi
export ANDROID_HOME
export ANDROID_SDK_ROOT
export NDK_HOME
export ANDROID_NDK_HOME
export ANDROID_NDK
export CMAKE_TOOLCHAIN_FILE

if [[ ! -d "$ANDROID_GEN_DIR" ]]; then
  pnpm --dir "$DESKTOP_DIR" exec tauri android init --ci
fi

if [[ -d "$ICON_SRC_DIR" ]]; then
  while IFS= read -r icon_file; do
    rel="${icon_file#"$ICON_SRC_DIR"/}"
    mkdir -p "$ANDROID_RES_DIR/$(dirname "$rel")"
    cp "$icon_file" "$ANDROID_RES_DIR/$rel"
  done < <(find "$ICON_SRC_DIR" -type f | sort)
fi

BUNDLE_FLAG="--apk"
if [[ "$ARTIFACT" == "aab" ]]; then
  BUNDLE_FLAG="--aab"
fi

MODE_FLAG=()
if [[ "$MODE" == "debug" ]]; then
  MODE_FLAG=(--debug)
fi
TARGET="${ANDROID_TARGET:-aarch64}"

pnpm --dir "$DESKTOP_DIR" exec tauri android build "$BUNDLE_FLAG" "${MODE_FLAG[@]}" --target "$TARGET"

if [[ "$ARTIFACT" != "apk" || "$MODE" != "release" ]]; then
  exit 0
fi

UNSIGNED_APK="$ANDROID_GEN_DIR/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
if [[ ! -f "$UNSIGNED_APK" ]]; then
  echo "Unsigned APK not found at expected path: $UNSIGNED_APK" >&2
  exit 1
fi

LATEST_BUILD_TOOLS="$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n1)"
ZIPALIGN="$LATEST_BUILD_TOOLS/zipalign"
APKSIGNER="$LATEST_BUILD_TOOLS/apksigner"
if [[ ! -x "$ZIPALIGN" || ! -x "$APKSIGNER" ]]; then
  echo "Could not locate zipalign/apksigner under $ANDROID_HOME/build-tools" >&2
  exit 1
fi

JAVA_HOME="${JAVA_HOME:-}"
if [[ -z "$JAVA_HOME" && -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]]; then
  JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
if [[ -n "$JAVA_HOME" ]]; then
  export JAVA_HOME
  export PATH="$JAVA_HOME/bin:$PATH"
fi

SIGNING_DIR="$TAURI_DIR/.signing"
SIGNING_ENV="$SIGNING_DIR/kiku-alpha-signing.env"
DEFAULT_KEYSTORE="$SIGNING_DIR/kiku-alpha-upload.jks"
mkdir -p "$SIGNING_DIR"

if [[ ! -f "$SIGNING_ENV" ]]; then
  STORE_PASSWORD="$(uuidgen | tr -d '-')"
  cat >"$SIGNING_ENV" <<EOF
KIKU_ANDROID_KEYSTORE_PATH=$DEFAULT_KEYSTORE
KIKU_ANDROID_KEY_ALIAS=kiku-alpha
KIKU_ANDROID_STORE_PASSWORD=$STORE_PASSWORD
EOF
fi

# shellcheck source=/dev/null
source "$SIGNING_ENV"
if [[ -z "${KIKU_ANDROID_KEYSTORE_PATH:-}" || -z "${KIKU_ANDROID_KEY_ALIAS:-}" || -z "${KIKU_ANDROID_STORE_PASSWORD:-}" ]]; then
  echo "Signing env is missing required variables in $SIGNING_ENV" >&2
  exit 1
fi

if [[ ! -f "$KIKU_ANDROID_KEYSTORE_PATH" ]]; then
  KEYTOOL_BIN="${KEYTOOL_BIN:-}"
  if [[ -z "$KEYTOOL_BIN" && -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/keytool" ]]; then
    KEYTOOL_BIN="$JAVA_HOME/bin/keytool"
  fi
  if [[ -z "$KEYTOOL_BIN" ]]; then
    KEYTOOL_BIN="$(command -v keytool || true)"
  fi
  if [[ -z "$KEYTOOL_BIN" ]]; then
    echo "keytool not found; cannot create Android signing key." >&2
    exit 1
  fi

  "$KEYTOOL_BIN" -genkeypair -v \
    -keystore "$KIKU_ANDROID_KEYSTORE_PATH" \
    -alias "$KIKU_ANDROID_KEY_ALIAS" \
    -keyalg RSA \
    -keysize 4096 \
    -validity 9125 \
    -storepass "$KIKU_ANDROID_STORE_PASSWORD" \
    -dname "CN=Kiku Alpha, OU=QA, O=Kiku, L=Chicago, ST=IL, C=US"
fi

APP_VERSION="$(node -e "console.log(require('$TAURI_CONF').version)")"
OUT_DIR="$ROOT_DIR/target/android"
mkdir -p "$OUT_DIR"
ALIGNED_APK="$OUT_DIR/Kiku_${APP_VERSION}_android_${TARGET}_release_aligned_unsigned.apk"
SIGNED_APK="$OUT_DIR/Kiku_${APP_VERSION}_android_${TARGET}_release_alpha_signed.apk"
rm -f "$ALIGNED_APK" "$SIGNED_APK"

"$ZIPALIGN" -f -p 4 "$UNSIGNED_APK" "$ALIGNED_APK"
"$APKSIGNER" sign \
  --ks "$KIKU_ANDROID_KEYSTORE_PATH" \
  --ks-key-alias "$KIKU_ANDROID_KEY_ALIAS" \
  --ks-pass "pass:$KIKU_ANDROID_STORE_PASSWORD" \
  --out "$SIGNED_APK" \
  "$ALIGNED_APK"
"$APKSIGNER" verify --verbose --print-certs "$SIGNED_APK"
shasum -a 256 "$SIGNED_APK"
echo "Signed release APK: $SIGNED_APK"
echo "Signing config: $SIGNING_ENV"

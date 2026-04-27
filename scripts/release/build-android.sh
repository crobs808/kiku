#!/usr/bin/env bash
set -euo pipefail

die() {
  echo "$1" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Required command not found: $cmd"
}

upsert_env_var() {
  local env_file="$1"
  local key="$2"
  local value="$3"
  local temp_file

  temp_file="$(mktemp)"
  awk -v key="$key" -v value="$value" '
    BEGIN { replaced = 0 }
    index($0, key "=") == 1 {
      print key "=" value
      replaced = 1
      next
    }
    { print }
    END {
      if (!replaced) {
        print key "=" value
      }
    }
  ' "$env_file" >"$temp_file"
  mv "$temp_file" "$env_file"
}

resolve_keytool_bin() {
  local candidate="${KEYTOOL_BIN:-}"
  if [[ -z "$candidate" && -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/keytool" ]]; then
    candidate="$JAVA_HOME/bin/keytool"
  fi
  if [[ -z "$candidate" ]]; then
    candidate="$(command -v keytool || true)"
  fi
  [[ -n "$candidate" ]] || die "keytool not found; cannot create or validate Android signing key."
  "$candidate" -help >/dev/null 2>&1 || die "keytool is not usable. Ensure a Java runtime is installed and JAVA_HOME is set."
  printf '%s\n' "$candidate"
}

ARTIFACT="${1:-}"
MODE="${2:-}"

if [[ "$ARTIFACT" != "apk" && "$ARTIFACT" != "aab" ]]; then
  die "Usage: ./scripts/release/build-android.sh <apk|aab> <debug|release>"
fi

if [[ "$MODE" != "debug" && "$MODE" != "release" ]]; then
  die "Usage: ./scripts/release/build-android.sh <apk|aab> <debug|release>"
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
TAURI_DIR="$DESKTOP_DIR/src-tauri"
TAURI_CONF="$TAURI_DIR/tauri.conf.json"
ANDROID_GEN_DIR="$TAURI_DIR/gen/android"
ANDROID_RES_DIR="$ANDROID_GEN_DIR/app/src/main/res"
ICON_SRC_DIR="$TAURI_DIR/icons/android"

if [[ ! -f "$DESKTOP_DIR/package.json" || ! -f "$TAURI_CONF" ]]; then
  die "Android build is not configured in apps/desktop."
fi

TARGET="${ANDROID_TARGET:-aarch64}"
case "$TARGET" in
  aarch64)
    RUST_TARGET_TRIPLE="aarch64-linux-android"
    TARGET_ABI="arm64-v8a"
    ;;
  armv7)
    RUST_TARGET_TRIPLE="armv7-linux-androideabi"
    TARGET_ABI="armeabi-v7a"
    ;;
  x86_64)
    RUST_TARGET_TRIPLE="x86_64-linux-android"
    TARGET_ABI="x86_64"
    ;;
  x86)
    RUST_TARGET_TRIPLE="i686-linux-android"
    TARGET_ABI="x86"
    ;;
  *)
    die "Unsupported ANDROID_TARGET: $TARGET (supported: aarch64, armv7, x86_64, x86)"
    ;;
esac

require_cmd pnpm
require_cmd node
require_cmd cargo
require_cmd rustup

# Keep mobile app icon assets aligned to the dedicated mobile logo source.
pnpm --dir "$DESKTOP_DIR" run icon:sync:mobile

if ! rustup target list --installed | grep -qx "$RUST_TARGET_TRIPLE"; then
  die "Missing Rust target $RUST_TARGET_TRIPLE. Run: rustup target add $RUST_TARGET_TRIPLE"
fi

ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
NDK_VERSION="${NDK_VERSION:-29.0.14206865}"
NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/$NDK_VERSION}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$NDK_HOME}"
ANDROID_NDK="${ANDROID_NDK:-$NDK_HOME}"
CMAKE_TOOLCHAIN_FILE="${CMAKE_TOOLCHAIN_FILE:-$NDK_HOME/build/cmake/android.toolchain.cmake}"
CMDLINE_TOOLS_BIN="${CMDLINE_TOOLS_BIN:-/opt/homebrew/share/android-commandlinetools/cmdline-tools/latest/bin}"

[[ -d "$ANDROID_HOME" ]] || die "ANDROID_HOME does not exist: $ANDROID_HOME"
[[ -d "$NDK_HOME" ]] || die "NDK_HOME does not exist: $NDK_HOME"
[[ -f "$CMAKE_TOOLCHAIN_FILE" ]] || die "Android CMake toolchain file not found: $CMAKE_TOOLCHAIN_FILE"

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

pnpm --dir "$DESKTOP_DIR" exec tauri android build "$BUNDLE_FLAG" "${MODE_FLAG[@]}" --target "$TARGET"

if [[ "$ARTIFACT" != "apk" || "$MODE" != "release" ]]; then
  exit 0
fi

APK_OUTPUT_DIR="$ANDROID_GEN_DIR/app/build/outputs/apk"
[[ -d "$APK_OUTPUT_DIR" ]] || die "APK output directory not found: $APK_OUTPUT_DIR"

mapfile -t UNSIGNED_APK_CANDIDATES < <(find "$APK_OUTPUT_DIR" -type f -name "*-release-unsigned.apk" | sort)
if [[ "${#UNSIGNED_APK_CANDIDATES[@]}" -eq 0 ]]; then
  die "No unsigned release APK found under: $APK_OUTPUT_DIR"
fi

UNSIGNED_APK=""
for candidate in "${UNSIGNED_APK_CANDIDATES[@]}"; do
  if [[ "$candidate" == *"/$TARGET_ABI/"* ]]; then
    UNSIGNED_APK="$candidate"
    break
  fi
done
if [[ -z "$UNSIGNED_APK" ]]; then
  for candidate in "${UNSIGNED_APK_CANDIDATES[@]}"; do
    if [[ "$candidate" == *"/universal/"* ]]; then
      UNSIGNED_APK="$candidate"
      break
    fi
  done
fi
if [[ -z "$UNSIGNED_APK" ]]; then
  UNSIGNED_APK="${UNSIGNED_APK_CANDIDATES[0]}"
fi
echo "Using unsigned APK: $UNSIGNED_APK"

BUILD_TOOLS_DIR="$ANDROID_HOME/build-tools"
[[ -d "$BUILD_TOOLS_DIR" ]] || die "Android build-tools directory not found: $BUILD_TOOLS_DIR"

LATEST_BUILD_TOOLS="$(find "$BUILD_TOOLS_DIR" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n1)"
[[ -n "$LATEST_BUILD_TOOLS" ]] || die "No Android build-tools versions found under $BUILD_TOOLS_DIR"

ZIPALIGN="$LATEST_BUILD_TOOLS/zipalign"
APKSIGNER="$LATEST_BUILD_TOOLS/apksigner"
if [[ ! -x "$ZIPALIGN" || ! -x "$APKSIGNER" ]]; then
  die "Could not locate zipalign/apksigner under $BUILD_TOOLS_DIR"
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
SIGNING_ENV="${KIKU_ANDROID_SIGNING_ENV_PATH:-$SIGNING_DIR/kiku-alpha-signing.env}"
DEFAULT_KEYSTORE="$SIGNING_DIR/kiku-alpha-upload.jks"
mkdir -p "$SIGNING_DIR"

if [[ ! -f "$SIGNING_ENV" ]]; then
  STORE_PASSWORD="$(uuidgen | tr -d '-')"
  cat >"$SIGNING_ENV" <<EOF
KIKU_ANDROID_KEYSTORE_PATH=$DEFAULT_KEYSTORE
KIKU_ANDROID_KEY_ALIAS=kiku-alpha
KIKU_ANDROID_STORE_PASSWORD=$STORE_PASSWORD
KIKU_ANDROID_KEY_PASSWORD=$STORE_PASSWORD
EOF
fi

# shellcheck source=/dev/null
source "$SIGNING_ENV"
if [[ -z "${KIKU_ANDROID_KEYSTORE_PATH:-}" || -z "${KIKU_ANDROID_KEY_ALIAS:-}" || -z "${KIKU_ANDROID_STORE_PASSWORD:-}" ]]; then
  die "Signing env is missing required variables in $SIGNING_ENV"
fi
KIKU_ANDROID_KEY_PASSWORD="${KIKU_ANDROID_KEY_PASSWORD:-$KIKU_ANDROID_STORE_PASSWORD}"
KEYTOOL_BIN="$(resolve_keytool_bin)"

if [[ ! -f "$KIKU_ANDROID_KEYSTORE_PATH" ]]; then
  "$KEYTOOL_BIN" -genkeypair -v \
    -keystore "$KIKU_ANDROID_KEYSTORE_PATH" \
    -alias "$KIKU_ANDROID_KEY_ALIAS" \
    -storetype PKCS12 \
    -keyalg RSA \
    -keysize 4096 \
    -validity 9125 \
    -storepass "$KIKU_ANDROID_STORE_PASSWORD" \
    -keypass "$KIKU_ANDROID_STORE_PASSWORD" \
    -dname "CN=Kiku Alpha, OU=QA, O=Kiku, L=Chicago, ST=IL, C=US"
  KIKU_ANDROID_KEY_PASSWORD="$KIKU_ANDROID_STORE_PASSWORD"
  upsert_env_var "$SIGNING_ENV" "KIKU_ANDROID_KEY_PASSWORD" "$KIKU_ANDROID_KEY_PASSWORD"
fi

KEYSTORE_INFO_FILE="$(mktemp)"
if ! "$KEYTOOL_BIN" -list \
  -keystore "$KIKU_ANDROID_KEYSTORE_PATH" \
  -storepass "$KIKU_ANDROID_STORE_PASSWORD" \
  >"$KEYSTORE_INFO_FILE" 2>/dev/null; then
  rm -f "$KEYSTORE_INFO_FILE"
  die "Failed to open keystore with configured store password: $KIKU_ANDROID_KEYSTORE_PATH"
fi

KEYSTORE_TYPE="$(awk -F': ' '/^Keystore type:/{print $2; exit}' "$KEYSTORE_INFO_FILE" | tr '[:lower:]' '[:upper:]')"
rm -f "$KEYSTORE_INFO_FILE"
if [[ "$KEYSTORE_TYPE" == "PKCS12" && "$KIKU_ANDROID_KEY_PASSWORD" != "$KIKU_ANDROID_STORE_PASSWORD" ]]; then
  echo "Detected PKCS12 keystore; forcing key password to match store password from $SIGNING_ENV"
  KIKU_ANDROID_KEY_PASSWORD="$KIKU_ANDROID_STORE_PASSWORD"
  upsert_env_var "$SIGNING_ENV" "KIKU_ANDROID_KEY_PASSWORD" "$KIKU_ANDROID_KEY_PASSWORD"
fi

APP_VERSION="$(node -e "console.log(require('$TAURI_CONF').version)")"
OUT_DIR="$ROOT_DIR/target/android"
mkdir -p "$OUT_DIR"
ALIGNED_APK="$OUT_DIR/Kiku_${APP_VERSION}_android_${TARGET}_release_aligned_unsigned.apk"
SIGNED_APK="$OUT_DIR/Kiku_${APP_VERSION}_android_${TARGET}_release_alpha_signed.apk"
rm -f "$ALIGNED_APK" "$SIGNED_APK"

"$ZIPALIGN" -f -p 4 "$UNSIGNED_APK" "$ALIGNED_APK"
sign_apk() {
  local key_password="$1"
  rm -f "$SIGNED_APK"
  "$APKSIGNER" sign \
    --ks "$KIKU_ANDROID_KEYSTORE_PATH" \
    --ks-key-alias "$KIKU_ANDROID_KEY_ALIAS" \
    --ks-pass "pass:$KIKU_ANDROID_STORE_PASSWORD" \
    --key-pass "pass:$key_password" \
    --out "$SIGNED_APK" \
    "$ALIGNED_APK"
}

SIGN_ERR_FILE="$(mktemp)"
if ! sign_apk "$KIKU_ANDROID_KEY_PASSWORD" 2>"$SIGN_ERR_FILE"; then
  if [[ "$KIKU_ANDROID_KEY_PASSWORD" != "$KIKU_ANDROID_STORE_PASSWORD" ]]; then
    echo "Primary key password failed; retrying APK signing with store password."
    if sign_apk "$KIKU_ANDROID_STORE_PASSWORD" 2>>"$SIGN_ERR_FILE"; then
      KIKU_ANDROID_KEY_PASSWORD="$KIKU_ANDROID_STORE_PASSWORD"
      upsert_env_var "$SIGNING_ENV" "KIKU_ANDROID_KEY_PASSWORD" "$KIKU_ANDROID_KEY_PASSWORD"
    else
      cat "$SIGN_ERR_FILE" >&2
      rm -f "$SIGN_ERR_FILE"
      die "Failed to sign APK with configured keystore credentials."
    fi
  else
    cat "$SIGN_ERR_FILE" >&2
    rm -f "$SIGN_ERR_FILE"
    die "Failed to sign APK with configured keystore credentials."
  fi
fi
rm -f "$SIGN_ERR_FILE"

"$APKSIGNER" verify --verbose --print-certs "$SIGNED_APK"
shasum -a 256 "$SIGNED_APK"
echo "Signed release APK: $SIGNED_APK"
echo "Signing config: $SIGNING_ENV"

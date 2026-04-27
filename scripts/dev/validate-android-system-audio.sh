#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEFAULT_APK="$ROOT_DIR/target/android/Kiku_0.1.7_android_aarch64_release_alpha_signed.apk"
APK_PATH="${1:-$DEFAULT_APK}"
ADB_SERIAL="${ADB_SERIAL:-}"
PACKAGE_NAME="com.kiku.desktop"
MAIN_ACTIVITY="com.kiku.desktop.MainActivity"
LOG_OUT="$ROOT_DIR/target/android/validation-system-audio.log"

die() {
  echo "$1" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "Missing required command: $cmd"
}

adb_cmd() {
  if [[ -n "$ADB_SERIAL" ]]; then
    adb -s "$ADB_SERIAL" "$@"
  else
    adb "$@"
  fi
}

pick_single_device_if_needed() {
  if [[ -n "$ADB_SERIAL" ]]; then
    return
  fi

  local connected
  mapfile -t connected < <(adb devices | awk 'NR>1 && $2=="device" {print $1}')
  if [[ "${#connected[@]}" -eq 0 ]]; then
    die "No authorized Android device found. Connect a device and accept USB debugging."
  fi
  if [[ "${#connected[@]}" -gt 1 ]]; then
    die "Multiple devices found. Re-run with ADB_SERIAL=<device-id>."
  fi
  ADB_SERIAL="${connected[0]}"
}

require_cmd adb
[[ -f "$APK_PATH" ]] || die "APK not found: $APK_PATH"

pick_single_device_if_needed
echo "Using device: $ADB_SERIAL"

SDK_LEVEL="$(adb_cmd shell getprop ro.build.version.sdk | tr -d '\r')"
if [[ -z "$SDK_LEVEL" ]]; then
  die "Failed to read device SDK level."
fi
if (( SDK_LEVEL < 29 )); then
  die "Android SDK $SDK_LEVEL detected. Playback capture requires Android 10+ (SDK 29+)."
fi
echo "Device SDK level: $SDK_LEVEL"

echo "Installing APK: $APK_PATH"
adb_cmd install -r "$APK_PATH"

echo "Granting microphone permission"
adb_cmd shell pm grant "$PACKAGE_NAME" android.permission.RECORD_AUDIO || true

echo "Clearing previous app process + logs"
adb_cmd shell am force-stop "$PACKAGE_NAME" || true
adb_cmd logcat -c || true

echo "Launching app"
adb_cmd shell am start -W -n "$PACKAGE_NAME/$MAIN_ACTIVITY"

echo "Waiting for startup logs"
sleep 3

mkdir -p "$(dirname "$LOG_OUT")"
adb_cmd logcat -d >"$LOG_OUT"

echo
echo "Collected logcat: $LOG_OUT"
echo "Key lines:"
grep -E "MainActivity|MediaProjection|AudioPlayback|AudioRecord|kiku|System audio|permission" "$LOG_OUT" | tail -n 80 || true
echo
echo "Manual on-device validation steps:"
echo "1. In Kiku, tap System to trigger playback-capture permission."
echo "2. Accept Android capture consent prompt."
echo "3. Start listening in Kiku with System enabled."
echo "4. Play audio from another app and confirm transcript lines update."
echo "5. Re-run this script to capture fresh logs after reproducing."

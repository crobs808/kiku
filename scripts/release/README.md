# Release Scripts

Release automation and packaging helpers.

## Android Packaging

`./scripts/release/build-android.sh <apk|aab> <debug|release>`

- Uses `apps/desktop` as the Android/Tauri source of truth.
- Regenerates mobile icon assets from `assets/kiku-app-logo-mobile.png` before each build.
- Syncs Android icon assets from `apps/desktop/src-tauri/icons/android`.
- Builds with `ANDROID_TARGET=aarch64` by default (`armv7`, `x86_64`, and `x86` also supported).
- For release APKs, produces:
  - unsigned aligned APK
  - signed alpha APK in `target/android/`

### Prerequisites

- `pnpm`, `node`, `cargo`, and `rustup` installed.
- Rust Android target installed for selected `ANDROID_TARGET`:
  - `aarch64` -> `aarch64-linux-android`
  - `armv7` -> `armv7-linux-androideabi`
  - `x86_64` -> `x86_64-linux-android`
  - `x86` -> `i686-linux-android`
- Android SDK + NDK installed (`ANDROID_HOME` defaults to `$HOME/Library/Android/sdk`).
- Android build-tools installed (`zipalign` + `apksigner`).
- Java runtime available for keystore generation/signing (`keytool`).

### Build Commands

- Release signed APK: `pnpm build:android:apk:release`
- Release AAB: `pnpm build:android:aab:release`
- Debug APK: `pnpm build:android:apk:debug`

Optional environment overrides:

- `ANDROID_TARGET` (`aarch64`, `armv7`, `x86_64`, `x86`)
- `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `NDK_HOME`, `NDK_VERSION`
- `KIKU_ANDROID_SIGNING_ENV_PATH` (custom signing env file location)

### Signing

On first release APK build, a local signing key is generated in:

- `apps/desktop/src-tauri/.signing/kiku-alpha-upload.jks`
- `apps/desktop/src-tauri/.signing/kiku-alpha-signing.env`

Signing env variables:

- `KIKU_ANDROID_KEYSTORE_PATH`
- `KIKU_ANDROID_KEY_ALIAS`
- `KIKU_ANDROID_STORE_PASSWORD`
- `KIKU_ANDROID_KEY_PASSWORD` (defaults to store password if omitted)

These files are git-ignored and should be backed up privately. Reuse the same key for all alpha builds if you want in-place app updates for testers.

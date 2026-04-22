# Release Scripts

Release automation and packaging helpers.

## Android Packaging

`./scripts/release/build-android.sh <apk|aab> <debug|release>`

- Uses `apps/desktop` as the Android/Tauri source of truth.
- Automatically syncs Android icon assets from `apps/desktop/src-tauri/icons/android`.
- Builds with `ANDROID_TARGET=aarch64` by default.
- For release APKs, produces:
  - unsigned aligned APK
  - signed alpha APK in `target/android/`

### Signing

On first release APK build, a local signing key is generated in:

- `apps/desktop/src-tauri/.signing/kiku-alpha-upload.jks`
- `apps/desktop/src-tauri/.signing/kiku-alpha-signing.env`

These files are git-ignored and should be backed up privately. Reuse the same key for all alpha builds if you want in-place app updates for testers.

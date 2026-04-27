# Android Shell

Android builds are currently driven by the Tauri mobile target in
`apps/desktop/src-tauri` and packaged through:

- `./scripts/release/build-android.sh apk release`
- `./scripts/release/build-android.sh aab release`

Current Android status:

- signed alpha APK generation is working end-to-end
- UI/session/model/cloud settings flows match desktop behavior
- microphone permission flow is handled in `MainActivity.kt`
- system playback-capture mode is implemented via `MediaProjection` + `AudioPlaybackCaptureConfiguration`
- mobile icon source for Android/iOS builds is `assets/kiku-app-logo-mobile.png`

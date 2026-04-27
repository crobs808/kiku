# Developer Commands

Useful terminal commands for day-to-day Kiku development.

## Local Desktop Development (macOS)

Start the local desktop app (Tauri + UI dev mode):

```bash
pnpm start
```

If startup fails because port `1420` is already in use:

```bash
pkill -f "tauri dev|vite --port 1420|kiku-desktop" || true
pnpm start
```

## Android: Install Release APK to Paired Wi-Fi Debugging Device

Install the latest signed alpha APK:

```bash
adb install -r "$(ls -t target/android/Kiku_*_android_*_release_alpha_signed.apk | head -n1)"
```

If more than one device is connected, target a specific serial:

```bash
adb devices -l
adb -s <device-serial> install -r "$(ls -t target/android/Kiku_*_android_*_release_alpha_signed.apk | head -n1)"
```

## macOS System Audio Permission Workflow (Recommended)

Build a real app bundle:

```bash
pnpm --dir apps/desktop exec tauri build --bundles app
```

Install to `Applications`:

```bash
cp -R target/release/bundle/macos/Kiku.app /Applications/
```

Launch from `Applications`:

```bash
open -a /Applications/Kiku.app
```

If macOS previously denied Screen Recording and no prompt appears, reset TCC and relaunch:

```bash
tccutil reset ScreenCapture com.kiku.desktop
```

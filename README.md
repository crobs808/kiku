# Kiku

Kiku is a local-first live translation and captioning app for confidential conversations.

## Current Phase

This repository is in early scaffold state, following `docs/kiku-design-plan.md` Phase 0:

- Rust workspace and crate boundaries
- Desktop shell scaffold (Tauri + React + TypeScript)
- Floating caption window baseline
- Settings/model/capture/privacy interfaces and session state stubs

## Repository Layout

```text
apps/         Platform shells (desktop now, Android later)
crates/       Rust domain crates
native/       Platform-specific plugins and adapters
models/       Local model manifests and metadata
docs/         Product, architecture, UX, and security docs
scripts/      Dev/release/packaging scripts
tests/        Integration tests and fixtures
```

## Tooling

- Package manager: `pnpm`
- Workspace file: `pnpm-workspace.yaml`
- Root startup command: `pnpm start`

## Local Development

1. Install JavaScript dependencies:

```bash
pnpm install
```

2. Start the desktop live preview (UI + Tauri shell):

```bash
pnpm start
```

`pnpm start` delegates to `apps/desktop` and currently runs:

1. `pnpm run icon:sync` (refreshes Tauri icons from `assets/kiku-app-logo-aqua.png`)
2. `cargo clean -p kiku-desktop`
3. `tauri dev`

If `pnpm start` fails with `Port 1420 is already in use`, stop any stale run and retry:

```bash
pkill -f "tauri dev|vite --port 1420|kiku-desktop" || true
pnpm start
```

## Bundle-Ready Prototype Commands

- Build UI bundle only:

```bash
pnpm ui:build
```

- Build macOS app + DMG bundle:

```bash
pnpm bundle:mac:prototype
```

`pnpm bundle:mac:prototype` now auto-increments the release version before building.

## Build Commands By Target

Run these from repo root.

- macOS ARM64 DMG: `pnpm build:mac:arm64:dmg`
- macOS Universal DMG: `pnpm build:mac:universal:dmg`
- Windows x64 EXE (NSIS): `pnpm build:win:x64:exe`
- Windows x64 MSI: `pnpm build:win:x64:msi`
- Linux x64 AppImage: `pnpm build:linux:x64:appimage`
- Android APK (debug): `pnpm build:android:apk:debug`
- Android APK (release): `pnpm build:android:apk:release`
- Android AAB (release): `pnpm build:android:aab:release`

Convenience aliases:

- `pnpm build:mac` -> `pnpm build:mac:arm64:dmg`
- `pnpm build:win` -> `pnpm build:win:x64:exe`
- `pnpm build:linux` -> `pnpm build:linux:x64:appimage`
- `pnpm build:android` -> `pnpm build:android:apk:release`

## Auto Semver For Installer Builds

Installer build commands automatically bump semver before packaging:

- `pnpm build:mac:*`
- `pnpm build:win:*`
- `pnpm build:linux:*`
- `pnpm bundle:mac:prototype`

Use this to preview the next version without changing files:

```bash
pnpm version:next
```

Manual apply without building:

```bash
pnpm version:prepare
```

How bumping works:

- `BREAKING CHANGE:` footer or `type(scope)!:` commit -> **major**
- `feat:` commit -> **minor**
- everything else -> **patch**
- if there are no new commits since the previous release baseline, it still increments **patch** so each installer export gets a new version.

Release baseline detection:

- prefers latest semver git tag (`vX.Y.Z`)
- if no tag exists yet, falls back to the last commit that changed version files

Tip for GitHub binary releases:

1. Merge to `main` using Conventional Commit messages (`feat:`, `fix:`, etc.)
2. Run a build command from repo root (auto-bumps version)
3. Commit the version-file changes
4. Tag and push (example: `git tag v0.1.7 && git push origin main --tags`)
5. Upload the generated installer artifact from `target/release/bundle/...` to GitHub Releases

VS Code integration:

- Run `Tasks: Run Task` and use:
- `Kiku: Next Version Preview`
- `Kiku: Build macOS ARM64 DMG (Auto-Version)`
- `Kiku: Build macOS Universal DMG (Auto-Version)`

Notes:

- Desktop packaging is most reliable on native OS runners (macOS for `.dmg`, Windows for `.exe`/`.msi`, Linux for `.AppImage`).
- Android shell is currently scaffold-only in this repo; Android build commands are wired now and will fail with a clear message until `apps/android` is fully initialized.

## Local ASR Model (JP -> EN)

Live Japanese-to-English transcription uses a local Whisper model file.

On startup, if no model is found, Kiku now opens an in-app model manager with downloadable model options, WER/size guidance, and progress tracking. The default recommended choice is `Whisper Large v3` for highest accuracy.

From the main top bar, you can:
- open `Models` to manage downloads/deletions
- switch the active installed model before starting a new listening session

Place one of these files in `models/` (or set `KIKU_WHISPER_MODEL` to a full path):

- `models/ggml-base.bin`
- `models/ggml-small.bin`
- `models/ggml-medium.bin`
- `models/ggml-large-v3.bin`

Manual developer helper command:

```bash
./scripts/dev/fetch-whisper-model.sh large-v3
```

Without a local model, startup enters `Model Missing`, `Start Listening` is disabled, and the app shows setup guidance.

## Cloud Translation (Accuracy Mode)

The default translation backend in this prototype is local/stub quality. For production-grade translation quality, enable Google Cloud Translate:

```bash
export KIKU_TRANSLATION_PROVIDER=google_cloud
export KIKU_GOOGLE_TRANSLATE_API_KEY=your_api_key
pnpm start
```

If `KIKU_GOOGLE_TRANSLATE_API_KEY` is present and `KIKU_TRANSLATION_PROVIDER` is unset, Kiku will auto-enable Google Cloud Translate.

You can also configure providers and key from the app UI via the `Cloud` button in the top bar.

## Cloud ASR (Optional)

For higher ASR accuracy than local models in some environments, you can route speech recognition through Google Cloud Speech-to-Text:

```bash
export KIKU_ASR_PROVIDER=google_cloud
export KIKU_GOOGLE_SPEECH_API_KEY=your_api_key
pnpm start
```

`KIKU_GOOGLE_API_KEY` or `KIKU_GOOGLE_TRANSLATE_API_KEY` can also be used as a fallback key source.

Note: Google Cloud ASR is usage-based billing with limited free-tier allowances, not unlimited free usage.

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

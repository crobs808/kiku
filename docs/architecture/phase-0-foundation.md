# Phase 0 Foundation Notes

Initial scaffold implemented from the design plan:

- Rust workspace with modular crates for core, audio, asr, translation, transcript, model, settings, privacy, visualizer, and platform abstraction
- Desktop shell scaffold at `apps/desktop` using Tauri + React/TypeScript
- Desktop shell build/dev flow migrated to `pnpm` workspace commands
- Session lifecycle state machine in `kiku-core`
- In-memory model/settings/capture/privacy adapters for bring-up
- Placeholder caption stream UI with start/stop and save/discard flow hooks
- Native plugin interface placeholder for macOS capture
- Mic capture bring-up using `cpal` with live input activity level surfaced to UI

## Current Assumptions

- `kiku-desktop` boots in `ready` by default with a stub-installed model to unblock UI plumbing.
- Real model install flow and model-missing first-launch path will be introduced next.
- Transcript save currently returns plain text payload to UI; file save dialog integration is pending.

## Next Technical Slice

1. Move settings from in-memory to file-backed store in Rust.
2. Replace placeholder caption stream with Rust events emitted from session pipeline.
3. Integrate real transcription path (mic audio -> ASR -> caption updates).
4. Add macOS system-audio capture backend and merge policy with mic.
5. Harden macOS prototype bundling pipeline (`pnpm bundle:mac:prototype`) with production icon assets and signing strategy.

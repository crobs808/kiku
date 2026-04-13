# 2026-04-10 Tech Debt / Performance Finds

## Scope
Performance/code-tightness review across the current `kiku` repo, with emphasis on low-risk improvements that preserve behavior.

## Completed In This Pass
- Replaced front-drain audio buffering with deque-backed logic in core listening path.
- Replaced per-inference thread spawn with a single ASR worker thread per listening session.
- Added non-overlapping UI polling guards to prevent stacked IPC calls during listening/model polling/stopping.

## Remaining Findings (Backlog)

### 1) Audio callback alloc churn can be reduced
- Priority: Medium
- Area: `crates/kiku-platform/src/lib.rs`
- Current hotspot:
  - `interleaved_to_mono` builds temporary vectors per callback and uses a frame accumulator.
  - Conversion paths (`i16`, `u16`) allocate intermediary mono vectors before append.
- Why it matters:
  - Audio callbacks are frequent and should minimize allocations.
- Suggested change:
  - Use `chunks_exact(channel_count)` and pre-size output buffer.
  - Consider reusing scratch buffers in callback state.

### 2) Inference window still copies into a fresh vector every poll
- Priority: Medium
- Area: `crates/kiku-core/src/controller.rs`
- Current hotspot:
  - The inference window is collected into a new `Vec<f32>` before request dispatch.
- Why it matters:
  - Copy cost grows with window size and poll cadence.
- Suggested change:
  - Move to a reusable scratch window buffer in controller state.
  - Optionally evolve `AsrRequest` to support borrowed audio (`Cow<[f32]>`) when feasible.

### 3) Model inventory polling still does repeated `exists()` checks
- Priority: Medium
- Area: `apps/desktop/ui/src/App.tsx`, `apps/desktop/src-tauri/src/main.rs`
- Current hotspot:
  - While model modal is visible, polling repeatedly requests inventory.
  - Inventory computation checks filesystem existence for each model card each poll.
- Why it matters:
  - Unnecessary IO/IPC churn when modal is open but download state is idle.
- Suggested change:
  - Poll download progress only while a download is active.
  - Refresh inventory only on model actions and download completion/cancel.

### 4) Transcript export builds many intermediate strings
- Priority: Low
- Area: `crates/kiku-transcript/src/lib.rs`
- Current hotspot:
  - `map + format + collect + join` allocates multiple intermediates.
- Why it matters:
  - Minor, but easy win for long transcript saves.
- Suggested change:
  - Preallocate one `String` and append lines directly.

### 5) Download loop uses large stack buffer
- Priority: Low
- Area: `apps/desktop/src-tauri/src/main.rs`
- Current hotspot:
  - `[0u8; 64 * 1024]` stack allocation in download worker.
- Why it matters:
  - Not usually harmful, but avoidable and flagged by lint.
- Suggested change:
  - Move to heap-backed `Vec<u8>` / boxed slice.

### 6) Atomic ordering in privacy guard is stronger than required
- Priority: Low
- Area: `crates/kiku-privacy/src/lib.rs`
- Current hotspot:
  - `SeqCst` on all mode transitions/loads.
- Why it matters:
  - Small overhead and unnecessary global ordering for this use case.
- Suggested change:
  - Use acquire/release ordering semantics.

### 7) Dependency update backlog (version drift)
- Priority: Medium
- Area: Rust + UI toolchain dependencies
- Snapshot date: April 10, 2026
- Why it matters:
  - Staying current improves security posture, performance, and ecosystem compatibility.
- Current vs latest noted in audit:
  - Rust crates:
    - `whisper-rs` `0.11.1` -> `0.16.0`
    - `cpal` `0.15.3` -> `0.17.3`
    - `reqwest` direct dep `0.12.28` -> `0.13.2`
    - `thiserror` direct workspace line `1.x` while latest is `2.0.18`
  - JS/TS deps:
    - `react` `18.3.1` -> `19.2.5`
    - `react-dom` `18.3.1` -> `19.2.5`
    - `@types/react` `18.3.28` -> `19.2.14`
    - `@types/react-dom` `18.3.7` -> `19.2.3`
    - `@vitejs/plugin-react` `4.7.0` -> `6.0.1`
    - `typescript` `5.9.3` -> `6.0.2`
    - `vite` `5.4.21` -> `8.0.8`
  - Package manager:
    - `pnpm` pinned `10.8.1` -> latest `10.33.0`
- Suggested change:
  - Do staged upgrades in isolated batches:
    - Batch A: patch/minor-only upgrades with no major runtime changes.
    - Batch B: major UI toolchain upgrades (React 19 / Vite 8 / TS 6) with focused regression pass.
    - Batch C: audio/ASR stack upgrades (`cpal`, `whisper-rs`) with runtime validation on macOS capture/transcription paths.

## Notes
- These findings are intentionally scoped to behavior-preserving refactors.
- Functional changes (for example new model runtimes or algorithmic ASR changes) were excluded from this backlog list.

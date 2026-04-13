# Kiku Design Plan

## Document Status
- **Product Name:** Kiku
- **Version:** v0.1 MVP Design Plan
- **Primary Authoring Target:** VS Code + Codex
- **Primary Implementation Language:** Rust-first
- **Current Focus:** macOS MVP on Apple Silicon
- **Next Platform Target:** Android
- **Future Targets:** Windows, Linux, iOS

---

## 1. Product Summary

Kiku is a **local-first live translation and captioning app** for confidential conversations, meetings, and spoken audio. The primary MVP use case is:

- **Input:** Japanese speech
- **Output:** English live captions
- **Display:** A movable, resizable, always-on-top floating window beside meeting apps such as Microsoft Teams
- **Privacy Model:** Fully offline while listening and translating

Kiku should also support English and Japanese in both input and output directions for MVP, including same-language captioning.

The app must be designed so that it can eventually ship as:
- a native macOS desktop app
- an Android app
- later Windows and Linux desktop apps
- later iOS app support if desired

The codebase should live in a **monorepo** and produce **multiple platform-specific binaries** from shared core logic.

---

## 2. MVP Goals

### Primary MVP Goals
1. Ship a working **macOS Apple Silicon** desktop app first.
2. Translate **Japanese speech to English captions locally** with high accuracy and low latency.
3. Support **mic audio**, **system audio**, or **both** on macOS.
4. Display live translated captions in a **floating utility window**.
5. Buffer a transcript during the session and prompt the user to **save or discard** it on stop.
6. Keep all active listening and translation **offline**.
7. Build the architecture so Android can be added next with maximum code reuse.

### Secondary MVP Goals
1. Support English and Japanese in both directions.
2. Provide same-language captioning.
3. Include a simple visual activity/confidence widget.
4. Support persistent personalization settings.
5. Make the implementation heavily Rust-based so the project is a good Rust learning vehicle.

---

## 3. Non-Goals for MVP

These are intentionally out of scope for the first shipping version:

- Cloud ASR or cloud translation
- Speaker diarization with reliable person identity tracking
- Full transcript editor inside the app
- Full typed "Speak Mode" (text translate + TTS playback) implementation
- Full interactive "Learning Mode" implementation (guided pronunciation/scoring)
- Multi-language packs beyond English and Japanese
- iOS support in v1
- Intel Mac support in v1
- OS-level firewalling of the entire machine's network stack
- Deep Teams integration APIs
- Automatic meeting bot or virtual participant behavior
- Advanced collaborative sharing features

---

## 4. Product Principles

1. **Local first**
   - All capture, ASR, translation, and caption rendering happen on-device during listening.

2. **Confidential by design**
   - No cloud calls while listening.
   - No hidden uploads.
   - Clear offline mode indicators.

3. **Low-friction UX**
   - Easy to start.
   - No separate virtual audio cable setup required for the MVP user experience where possible.
   - Clear toggles for audio sources.

4. **Fast enough for live meetings**
   - Prioritize low-latency translation and good live readability.

5. **Rust first**
   - Use Rust for as much of the implementation as practical.
   - Keep native platform-specific code minimal and isolated.

6. **Scalable architecture**
   - Add languages, platforms, and model packs later without redesigning the core.

---

## 5. Confirmed Product Decisions

### Branding and app icon
- App name: **Kiku**
- Primary app icon mark: **聴**
- Product wordmark: **Kiku**
- Icon direction:
  - minimal, modern, high-contrast
  - designed to remain legible at small sizes
  - centered glyph in a simple bold container/background
  - include full-size icon, small-size simplified icon, monochrome variant, and future tray/menu bar variant
- Default visual direction:
  - dark-theme-friendly
  - strong accessibility contrast

### Naming
- App name: **Kiku**

### Platform rollout
- **Phase 1:** macOS on Apple Silicon
- **Phase 2:** Android
- **Phase 3:** Windows and Linux
- **Phase 4:** iOS if desired

### Audio inputs
- **macOS MVP:**
  - Mic input toggle
  - System audio input toggle
  - Both can run at once
- **Android MVP:**
  - Mic-only initially

### UI behavior
- Separate floating window
- Always on top
- Movable and resizable
- Optional transparency
- Live controls in the top bar
- Mode tabs in top bar: `Listen`, `Speak`, and `Learning` (`Speak`/`Learning` are planned, not in current MVP)
- Secondary settings menu for less frequently used options

### Transcript behavior
- Transcript buffer begins when the user clicks **Start Listening**
- On **Stop Listening**, user is prompted to:
  - Save transcript
  - Discard transcript
- Saved transcript contains:
  - translated captions only
  - timestamps
  - source tags
- Export format for MVP:
  - plain text only

### Source labeling
- Merged stream where possible
- Source labels use **icons only**

### Settings persistence
- User settings persist across launches

---

## 6. Technical Strategy

### Recommended stack

#### Core application architecture
- **Primary language:** Rust
- **Desktop/mobile shell:** Tauri 2
- **UI layer:** React + TypeScript
- **macOS native integration:** Swift only where necessary
- **Android native integration later:** Kotlin only where necessary

### Why this stack
This gives Kiku:
- a Rust-first core
- a path to desktop and mobile from one repository
- native packaging per platform
- a thin UI layer instead of a heavy Electron-style app
- enough flexibility to integrate macOS-native audio capture cleanly

### Rust-first ownership
Rust should own:
- session state machine
- audio pipeline orchestration
- chunking and buffering
- VAD integration
- ASR orchestration
- translation orchestration
- transcript buffering
- transcript export
- model download/install/update logic
- settings persistence
- privacy mode enforcement inside the app
- confidence calculation and visualization data
- command APIs exposed to the UI

### Minimal native code policy
Use native platform code only for:
- permissions
- system audio capture
- mic capture glue if needed
- platform-specific OS integrations that are not practical from Rust alone

---

## 7. Monorepo Structure

Proposed monorepo layout:

```text
kiku/
  README.md
  docs/
    design-plan.md
    architecture/
    security/
    ux/
  apps/
    desktop/
      src-tauri/
      ui/
    android/
      src-tauri/
      ui/
  crates/
    kiku-core/
    kiku-audio/
    kiku-asr/
    kiku-translate/
    kiku-transcript/
    kiku-models/
    kiku-settings/
    kiku-privacy/
    kiku-visualizer/
    kiku-platform/
  native/
    macos/
      KikuCapturePlugin/
    android/
      KikuAudioPlugin/
  models/
    manifests/
  scripts/
    dev/
    release/
    packaging/
  tests/
    integration/
    fixtures/
```

### Package responsibilities

#### `apps/desktop`
Tauri desktop shell, window behavior, packaging, app entrypoint.

#### `apps/android`
Tauri Android shell for future phase.

#### `crates/kiku-core`
Main orchestration crate. Session lifecycle, feature flags, app-level domain types.

#### `crates/kiku-audio`
Audio stream abstractions, device/source handling, buffer routing, chunking interface.

#### `crates/kiku-asr`
ASR adapter layer, model invocation, transcription and translation tasks.

#### `crates/kiku-translate`
Optional separate MT abstraction for future language scaling.

#### `crates/kiku-transcript`
Session transcript state, timestamp formatting, save/discard behavior.

#### `crates/kiku-models`
Model manifest parsing, installation, versioning, integrity checks, local storage locations.

#### `crates/kiku-settings`
User preferences and persistence.

#### `crates/kiku-privacy`
Offline-mode session policy, network-call guardrails, logging restrictions.

#### `crates/kiku-visualizer`
Activity/confidence widget data generation.

#### `native/macos`
Swift plugin(s) for ScreenCaptureKit and permission-sensitive integrations.

---

## 8. Target User Flows

### Flow A: First launch
1. User installs Kiku.
2. User opens Kiku.
3. App checks whether required local model pack is installed.
4. If not installed, app prompts the user to download the model pack.
5. User downloads and installs the model pack.
6. App shows readiness state.

### Flow B: Start live translation session
1. User opens Kiku.
2. User selects input/output language pair.
3. User enables mic toggle, system audio toggle, or both.
4. User positions the floating window beside Teams or another app.
5. User clicks **Start Listening**.
6. Kiku enters **Offline Mode Active** state.
7. Live translated captions appear in the floating window.
8. Transcript is buffered locally in the active session.

### Flow C: Stop session
1. User clicks **Stop Listening**.
2. Audio capture and inference stop.
3. User is prompted to:
   - Save transcript
   - Discard transcript
4. If save is chosen, system save dialog opens.
5. Transcript is written as plain text.
6. Session memory is cleared except for user-approved saved file.

### Flow D: Change UI settings
1. User opens settings menu.
2. User changes font size, color, theme, transparency, or other preferences.
3. Settings apply immediately where possible.
4. Settings persist across launches.

### Flow E: Planned Speak mode (post-MVP, discovery required)
1. User switches to the `Speak` tab.
2. User types source text (for example English).
3. User clicks `Translate`.
4. App shows translated output text below the source text (for example Japanese).
5. User clicks `Play`.
6. App speaks the translated text using local/offline TTS voice in the target language/accent.
7. User can revise source text and replay as needed.

### Flow F: Planned Learning mode (post-MVP, discovery required)
1. User switches to the `Learning` tab.
2. User selects a language direction (for example English -> Japanese practice).
3. User types a word or sentence they want to practice.
4. App shows the target-language text and optionally provides `Play` audio for reference pronunciation.
5. User clicks a mic icon and speaks the phrase.
6. App compares spoken output with target phrase and shows a match/score result (for example similarity percentage).
7. Future enhancement path: add pronunciation/accent quality scoring and targeted feedback hints.

---

## 9. Supported Language Modes for MVP

Kiku MVP should support the following modes:

1. English speech -> English captions
2. English speech -> Japanese captions
3. Japanese speech -> English captions
4. Japanese speech -> Japanese captions

### Priority order
1. Japanese speech -> English captions
2. English speech -> English captions
3. Japanese speech -> Japanese captions
4. English speech -> Japanese captions

---

## 10. ASR and Translation Model Strategy

### MVP default recommendation
Use **whisper.cpp** as the local speech runtime.

Use **Whisper large-v3** as the default macOS MVP model for the primary path:
- Japanese speech -> English live captions

### Why this is the recommended MVP default
- It is local-first.
- It has strong multilingual support.
- It is suitable for Apple Silicon performance optimization.
- It allows direct speech translation for the key Japanese-to-English use case.

### Performance presets
Planned model/runtime presets:

1. **Best Accuracy**
   - Whisper large-v3
   - Default for Japanese -> English

2. **Balanced / Lower Latency**
   - Whisper large-v3-turbo or equivalent future alternative
   - Optional, not default for translation-sensitive paths

3. **Experimental Japanese preset**
   - Japanese-focused alternative backend such as Kotoba-Whisper family
   - Behind feature flag until validated on real meeting audio

### MVP pipeline choice
For MVP, prefer the simplest reliable path:

- **Japanese speech -> English captions**
  - direct speech translation via Whisper-family pipeline

- **Same-language captioning**
  - direct transcription in the spoken language

### Future translation strategy
Keep the architecture open for a future cascade path:
- source speech -> source transcript -> local MT -> output captions

This allows broader language expansion later without rewriting the app.

---

## 11. Audio Capture Architecture

### macOS MVP requirements
Kiku on macOS must support:
- microphone capture
- system audio capture
- simultaneous capture when both toggles are on
- independent source enable/disable control

### Proposed architecture

#### Capture layer
- Swift native plugin for macOS capture integration
- System audio capture through platform-native framework(s)
- Mic capture through native APIs or Rust-compatible audio abstraction depending on implementation feasibility

#### Routing layer in Rust
- Normalize incoming audio into shared frame format
- Tag frames by source
- Buffer and chunk per source
- Merge or interleave streams for downstream captioning while preserving source metadata when possible

#### Inference input policy
- If one source is enabled, process that source directly
- If both are enabled:
  - support merged live caption stream
  - preserve source icon label where possible
  - avoid destructive mixing if it harms readability

### Important design note
The source system should be abstract enough that Android mic-only and later Windows/Linux capture backends can plug into the same Rust pipeline.

---

## 12. Session State Machine

Kiku should implement a clear session lifecycle.

### Session states
1. **Idle**
2. **Model Missing**
3. **Downloading Model**
4. **Ready**
5. **Listening**
6. **Stopping**
7. **Prompting Save/Discard**
8. **Saving Transcript**
9. **Error**

### Listening state behavior
When in `Listening` state:
- model updates disabled
- app-originated network activity disabled
- transcript buffer active
- live visual widget active if enabled
- source toggles either locked or change-safe depending on implementation stability

---

## 13. Offline and Privacy Design

### Privacy promises for MVP
1. No cloud ASR
2. No cloud translation
3. No active listening session network usage by the app
4. No persistent transcript saved unless the user explicitly saves on stop
5. No hidden transcript auto-save outside controlled session storage

### Clarified enforcement model
For MVP, Kiku should guarantee:
- **app-level no-network behavior during listening**
- no updater calls
- no telemetry calls
- no model download calls
- no cloud dependency in active session mode

### Offline mode UX
When listening is active, show:
- offline mode icon
- clear label such as `Offline Mode`
- subtle visual reassurance that processing is local

### Logging policy
- No raw audio recording in MVP
- No retained audio files unless explicitly added in a future feature
- Keep internal logs minimal and privacy-safe
- Redact or omit transcript contents from routine logs

---

## 14. Transcript Design

### During session
- Transcript exists in local app memory and/or controlled temp session storage only
- Contents include:
  - translated caption text
  - timestamp
  - source icon label metadata

### On stop
- Show modal or sheet:
  - Save transcript
  - Discard transcript

### Saved transcript format
- Plain text only for MVP

### Example export format
```text
[00:00:03] [icon] Good morning everyone.
[00:00:08] [icon] The next topic is the release timeline.
[00:00:15] [icon] Please review the updated proposal.
```

### Transcript exclusions
- No original-language text in MVP export
- No confidence values in saved transcript by default

---

## 15. UI and UX Design

### Main window concept
Kiku should behave like a floating utility window that sits beside or above a meeting application.

### Required characteristics
- always on top
- movable
- resizable
- clean caption area
- minimal control chrome
- subtle settings access
- optional transparency

### Main visible regions
0. **Mode tabs (planned)**
   - `Listen` tab for live audio captioning
   - `Learning` tab for guided speaking practice and scoring (planned, post-MVP)
   - `Speak` tab for typed translation + TTS playback (planned, post-MVP)

1. **Top control bar**
   - Start/Stop Listening
   - Mic toggle
   - System audio toggle
   - Language direction selector
   - Always-on-top state indicator if needed
   - Offline mode indicator when active

2. **Caption stream area**
   - rolling live translated captions
   - readable line spacing
   - source icons only
   - smooth updates without jitter

3. **Optional visual widget area**
   - listening/activity animation
   - confidence/translation activity visualization
   - user-toggleable

4. **Settings menu / panel**
   - theme mode
   - transparency slider
   - font family
   - font size
   - text color
   - visual widget enable/disable
   - model preset
   - transcript options

### UX principles
- Start should be one obvious action.
- Caption readability matters more than flashy visuals.
- Visual widget should be decorative but also informative.
- Settings should not clutter the main live-reading experience.

### Planned Speak mode note (discussion required before build)
Speak mode is approved as a planned feature direction, but it must not be implemented until a focused discovery pass defines:
- exact UX layout and tab behavior
- translation engine path for typed input
- local TTS runtime/provider and voice asset strategy
- latency, quality, and privacy acceptance criteria
- export/history behavior (if any)
- platform parity expectations (macOS first vs Android timing)

### Planned Learning mode note (discussion required before build)
Learning mode is approved as a planned feature direction, but it must not be implemented until a focused discovery pass defines:
- exact learning UX (single phrase drill vs session playlist/lesson flow)
- scoring model (string similarity baseline vs pronunciation/phoneme analysis)
- feedback depth (simple pass/fail vs detailed coaching hints)
- mic capture and privacy behavior for practice recordings (if any retention is allowed)
- reference-audio strategy (local TTS voices and asset packaging)
- progress tracking/history expectations and local data retention policy

---

## 16. Settings Persistence

The following settings should persist across launches:

- theme mode
- transparency
- font family
- font size
- text color
- visual widget enabled/disabled
- preferred input/output language pair
- preferred audio source toggles default state if appropriate
- preferred model preset
- window size and position if supported safely by platform

### Storage recommendation
- local config file managed by Rust
- clear schema versioning for future migrations

---

## 17. Visualization Widget

### Purpose
Add a subtle and optional visual element that gives the app a polished feel and shows that Kiku is actively listening, understanding, and translating.

### Design intent
This widget should communicate:
- audio activity
- inference activity
- confidence or stability estimate

### MVP constraints
- should not distract from captions
- should be toggleable off
- should not create significant GPU overhead
- data should come from Rust-side metrics rather than fake animation alone

### Candidate display styles
- waveform-like pulse
- radial activity ring
- small multi-stage graph with labels such as listen / understand / translate

---

## 18. Model Management

### Distribution strategy
Do not bundle the heaviest model directly in the smallest base installer.

### MVP plan
1. Ship a smaller base app.
2. On first launch, prompt user to download required model pack.
3. Install model pack locally.
4. Allow future local model updates when not listening.

### Current prototype status (April 9, 2026)
- App checks for a local Whisper model on startup.
- If no model is found, session state is `Model Missing`, `Start Listening` is disabled, and a model-manager modal is shown in-app.
- Model-manager modal supports model options with stats (size, approximate WER, best use), install progress, activation, and deletion.
- Model-manager catalog now includes both installable runtime options and planned non-Whisper candidate models, with clear `Available Now` vs `Planned` status badges.
- Each model card now shows clearer decision metadata (model family, language focus, latency profile, accuracy hint/WER context, and intent note) to reduce guesswork when selecting.
- Model-manager modal now includes active download cancel controls; closing (`Done`/`Later`) is disabled while download is active.
- Main top bar includes active-model selection and direct access to the model manager.
- On successful install or activation, the app transitions to `Ready` without requiring terminal commands.
- Model activation/install flow now guards against duplicate `Downloading Model` transitions to avoid stuck setup states.
- `Start Listening` is disabled when all audio sources are off, or when no installed model is available.
- If no model is installed, the transcript panel is visually locked with an in-app message directing users to the model manager.
- Language controls now include one-click input/output swap, and EN -> JA is enabled for prototype testing via local fallback translation logic.

### Requirements
- versioned model manifests
- integrity checks
- resumable or robust downloads if possible
- local storage path management
- clear disk usage display

### Language-pack scalability
In the future, only install model/language assets needed for selected languages when possible.

---

## 19. Performance Strategy

### Performance priorities
1. High translation accuracy
2. Low enough latency for live meetings
3. Aggressive use of local hardware where practical
4. Power consumption is not the primary concern

### Apple Silicon strategy
Optimize for Apple Silicon in v1.

Possible optimizations include:
- model/runtime choice tuned for Apple Silicon
- batching/chunk sizing experiments
- concurrency tuning in Rust
- optional use of platform acceleration paths where available
- minimizing unnecessary UI re-rendering

### Latency strategy
- tune chunk size carefully
- avoid excessive buffering
- incremental caption updates
- preserve readability while preventing flicker
- run ASR inference off the UI control path so `Stop Listening` and other controls remain responsive during heavy transcription
- reuse loaded ASR model context across inferences instead of reloading the model for each chunk

---

## 20. Cross-Platform Roadmap Strategy

### macOS first
macOS is the design center for MVP.

### Android second
Android should reuse:
- Rust core
- transcript engine
- model manager
- settings system
- inference abstractions
- most UI concepts

Android-specific work should focus on:
- mic capture
- permissions
- mobile window/layout adaptation
- thermal and memory constraints

### Windows/Linux later
Add platform capture backends while preserving the same Rust session and transcript engine.

### iOS later
Likely mic-only first. System audio capture expectations must remain conservative.

---

## 21. Security and Approval Notes

This app is intended for use in privileged or confidential conversations, so the design plan should explicitly support reviewability.

### Reviewer-facing design claims
- on-device processing during active sessions
- no cloud dependency while listening
- clear save/discard transcript flow
- no hidden auto-upload behavior
- no mandatory account requirement for local use

### Recommended future security appendix
Add a separate reviewer-oriented document later covering:
- data lifecycle diagram
- process boundaries
- threat model
- model update trust model
- local file storage paths
- logging policy
- network activity matrix by app state

---

## 22. Risks and Mitigations

### Risk 1: System audio capture complexity on macOS
**Mitigation:** isolate capture in a native plugin and keep the Rust pipeline backend-agnostic.

### Risk 2: Live translation latency too high with best-accuracy model
**Mitigation:** support a lower-latency preset and tune chunking aggressively.

### Risk 3: Japanese meeting speech quality varies by accents, overlap, and audio quality
**Mitigation:** keep model backend pluggable and benchmark on real meeting audio samples.

### Risk 4: UI becomes cluttered
**Mitigation:** keep the floating window minimal and move secondary controls into settings.

### Risk 5: Offline trust concerns from users
**Mitigation:** strong offline indicator, clear privacy language, and no listening-mode network behavior.

### Risk 6: Model files are large
**Mitigation:** separate model install flow from base app install.

---

## 23. Implementation Phases

### Phase 0: project foundation
- create monorepo
- configure Rust workspace
- create Tauri desktop shell
- build basic floating window
- persist simple settings

### Phase 1: macOS capture MVP
- implement mic capture
- implement system audio capture
- expose source toggles
- validate permissions UX

### Phase 2: ASR/translation MVP
- integrate whisper.cpp backend
- install and load default model
- implement JA -> EN live captions
- implement same-language captioning

### Phase 3: transcript and privacy
- session buffer
- save/discard flow
- plain text export
- offline mode state and UI

### Phase 4: polish
- always-on-top behavior
- transparency
- theme support
- font and color controls
- visual widget
- performance tuning

### Phase 5: Android follow-up
- port shell to Android
- mic-only flow
- mobile-safe UI layout
- model install on Android

### Phase 6: Speak mode (planned, post-discovery)
- add `Listen` vs `Speak` tab shell
- implement typed text translation workflow
- implement local TTS playback controls
- finalize UX after discovery decisions

### Phase 7: Learning mode (planned, post-discovery)
- add `Learning` tab shell and language-direction selector
- implement phrase prompt + user speech capture loop
- implement baseline match scoring and feedback UI
- evaluate optional pronunciation/accent scoring path
- finalize UX after discovery decisions

---

## 24. Suggested Internal Milestones

### Milestone A: Hello Kiku
- app launches
- floating window works
- settings persist

### Milestone B: Hear audio
- mic capture works
- system audio capture works
- source toggles wired

### Milestone C: See captions
- live Japanese -> English captions display
- caption stream stable and readable

### Milestone D: Trust it
- offline mode visible
- transcript save/discard flow implemented

### Milestone E: Ship candidate
- model install UX complete
- performance tuned on Apple Silicon
- app packaged for internal testing

---

## 25. Quality, Linting, and Incremental Verification

Kiku should be built with strict incremental quality controls so each implementation step is verified before the next step proceeds.

### Development quality requirements
1. Every meaningful code change should be small and incremental.
2. After each implementation prompt or coding step, the project should:
   - run linting
   - run formatting checks
   - run relevant unit tests
   - run relevant integration tests when affected
   - perform a build verification for the affected target where practical
3. New features should not be considered complete unless they pass the applicable quality gates.
4. Broken tests or lint failures should block further feature work until resolved.

### Recommended quality gates
- Rust formatting via `cargo fmt --check`
- Rust linting via `cargo clippy` with warnings treated as failures where practical
- Rust unit/integration tests via `cargo test`
- Frontend formatting and linting via project-standard formatter and linter
- Type checks for the frontend
- Tauri app build validation for the active platform during major milestones

### Verification strategy
- Prefer test-first or test-alongside development for core crates.
- Add regression tests whenever fixing a bug.
- Add snapshot or UI-behavior tests where useful for caption rendering and settings behavior.
- Verify state-machine transitions explicitly with automated tests.
- Verify transcript save/discard behavior with integration tests.
- Verify model-manager flows with test doubles where real downloads are not practical.

### Prompting guidance for Codex-assisted development
Each implementation prompt should instruct Codex to:
- make the smallest reasonable change
- explain what changed
- run the relevant checks
- report any failing checks clearly
- fix failures before moving on when possible
- avoid large speculative refactors unless explicitly requested

### Suggested verification levels by change type
- **Small Rust logic change:** fmt, clippy, targeted tests
- **State-machine or transcript change:** fmt, clippy, unit tests, integration tests
- **UI change:** frontend lint/typecheck, affected UI tests, desktop build smoke check when needed
- **Platform/audio change:** fmt, clippy, integration tests where possible, platform build validation, manual smoke checklist
- **Release milestone:** full lint, full tests, production build verification

### Manual smoke-check expectation
For features that are difficult to fully automate, include a lightweight manual verification checklist, especially for:
- mic capture
- system audio capture
- permissions prompts
- floating window behavior
- always-on-top behavior
- transcript save/discard flow
- offline-mode indicator behavior

## 26. Coding Guidance for Codex

### Project coding priorities
1. Keep Rust domain models explicit and strongly typed.
2. Prefer clean crate boundaries over one giant crate.
3. Make platform-specific code isolated behind traits/interfaces.
4. Build the app so backends can be swapped with minimal UI impact.
5. Treat privacy requirements as architecture-level constraints, not UI-only behavior.

### API design guidance
- expose small command surfaces from Rust to the UI
- avoid embedding business logic in React components
- keep transcript and session state canonical in Rust
- emit structured events from Rust for live UI updates

### Testing guidance
- unit test transcript formatting and session state machine
- integration test source toggle behavior
- regression test save/discard flows
- benchmark latency and memory for model presets

---

## 27. Open Technical Questions for Implementation

These do not block the design plan but will need decisions during implementation:

1. Exact whisper.cpp integration method:
   - linked library
   - subprocess wrapper
   - FFI bridge

2. Exact VAD choice for MVP
3. Exact macOS capture implementation split between Swift and Rust
4. Exact local storage paths for models and settings per platform
5. Exact visual style for the confidence/activity widget
6. Whether dual-source simultaneous processing should be mixed, interleaved, or independently chunked before merge
7. Speak mode information architecture: single-window tabs vs separate panel/window
8. Speak mode translation backend choice and whether it reuses ASR translation stack or separate MT path
9. Speak mode TTS backend/runtime and local voice asset packaging/update strategy
10. Speak mode privacy/logging policy for typed input and playback history
11. Learning mode scoring approach and success criteria (text similarity vs pronunciation model)
12. Learning mode UX depth (quick drill vs structured lesson progression)
13. Learning mode persistence/privacy policy for practice data and progress

---

## 28. Final Recommendation Summary

Build **Kiku** as a **Rust-first, local-first, monorepo-based live translation app** with:

- macOS Apple Silicon MVP first
- Tauri 2 shell
- Rust owning nearly all business logic
- minimal Swift for macOS-native capture integration
- whisper.cpp-based local speech runtime
- Whisper large-v3 as the initial best-accuracy default for Japanese-to-English live captions
- floating always-on-top caption window
- transcript save/discard flow on stop
- visible offline mode during active listening
- architecture ready for Android second and wider platform expansion later

This gives the best balance of:
- learning Rust deeply
- shipping something practical
- protecting privacy
- scaling to more platforms and languages later

---

## 29. Next Build Step

Start implementation by scaffolding:

1. Rust workspace
2. Tauri desktop app
3. floating always-on-top window
4. settings persistence
5. stub session state machine
6. placeholder caption stream UI
7. model manager shell
8. macOS capture plugin interface

Then iterate toward real audio capture and live translation.

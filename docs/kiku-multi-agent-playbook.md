# Kiku Multi-Agent Playbook (Codex + VS Code)

Last validated: April 15, 2026

Use this when you want reproducible multi-agent execution for this repo.

## 1) Repo Ownership Map

| Agent | Primary write ownership | Read scope | Must not write (default) |
| --- | --- | --- | --- |
| `rust_core` | `crates/kiku-core`, `crates/kiku-platform`, `crates/kiku-settings`, `crates/kiku-transcript`, `crates/kiku-translate`, `crates/kiku-models` | whole repo | `apps/desktop/ui` |
| `asr_agent` | `crates/kiku-asr`, `crates/kiku-audio`, `models/manifests`, `scripts/dev/fetch-whisper-model.sh` | whole repo | `apps/desktop/ui` |
| `ui_tauri` | `apps/desktop/ui`, `apps/desktop/src-tauri`, `native/macos/KikuCapturePlugin` | whole repo | `crates/kiku-asr` |
| `reviewer` | read-only all paths | whole repo | all writes |

If a task requires crossing these boundaries, explicitly reassign ownership in the kickoff prompt.

## 2) One-Time Local Setup (Optional)

Run from repo root:

```bash
mkdir -p .codex/agents
cat > .codex/config.toml <<'EOF'
[agents]
max_threads = 6
max_depth = 1
EOF
```

Create custom agents:

```bash
cat > .codex/agents/rust_core.toml <<'EOF'
name = "rust_core"
description = "Rust core/platform expert for Kiku orchestration and shared crates."
model = "gpt-5.4"
model_reasoning_effort = "high"
sandbox_mode = "workspace-write"
developer_instructions = """
Own changes in kiku-core, kiku-platform, kiku-settings, kiku-transcript, kiku-translate, and kiku-models.
Prefer small, testable changes.
Coordinate carefully when interfaces touch ASR or UI.
"""
EOF

cat > .codex/agents/asr_agent.toml <<'EOF'
name = "asr_agent"
description = "ASR/audio and model-integration specialist for Kiku."
model = "gpt-5.4"
model_reasoning_effort = "high"
sandbox_mode = "workspace-write"
developer_instructions = """
Own kiku-asr, kiku-audio, and model manifest integration paths.
Prioritize transcription quality, latency, and runtime reliability.
Avoid UI changes unless explicitly assigned.
"""
EOF

cat > .codex/agents/ui_tauri.toml <<'EOF'
name = "ui_tauri"
description = "Desktop UI + Tauri integration specialist."
model = "gpt-5.4-mini"
model_reasoning_effort = "medium"
sandbox_mode = "workspace-write"
developer_instructions = """
Own apps/desktop/ui, apps/desktop/src-tauri, and native/macos plugin integration points.
Preserve existing UX behavior unless task asks for redesign.
Avoid ASR crate edits unless explicitly assigned.
"""
EOF

cat > .codex/agents/reviewer.toml <<'EOF'
name = "reviewer"
description = "Read-only reviewer for regressions, safety, and missing tests."
model = "gpt-5.4"
model_reasoning_effort = "high"
sandbox_mode = "read-only"
developer_instructions = """
Review like an owner.
List findings first, sorted by severity, with exact file references.
Highlight missing tests and runtime regression risk.
"""
EOF
```

## 3) First 30 Minutes (Suggested Rhythm)

| Time | Action | Expected output |
| --- | --- | --- |
| 0-5 min | Define goal and acceptance criteria in parent prompt. | Single crisp success definition. |
| 5-10 min | Spawn discovery fanout (`rust_core`, `asr_agent`, `ui_tauri`) in read-only mode. | Architecture map, risks, candidate plan. |
| 10-20 min | Spawn writers with disjoint ownership for implementation. | Scoped diffs and validation results by owner. |
| 20-25 min | Run `reviewer` on merged result. | Severity-ordered findings and missing tests. |
| 25-30 min | Parent consolidates final summary and next actions. | One merged report with file refs and checks. |

## 4) Kiku Kickoff Prompts

### A) Feature Work Across Core + UI

```text
You are the parent orchestrator for kiku.

Spawn rust_core, ui_tauri, and reviewer.

Task: <feature description>

Ownership:
- rust_core: crates/kiku-core and related shared crates only
- ui_tauri: apps/desktop/ui and src-tauri only
- reviewer: read-only full repo

Wait policy:
1) rust_core and ui_tauri implement in parallel.
2) wait for both writers.
3) reviewer inspects merged diff.
4) return one consolidated summary.

Output format:
- changed files by owner
- checks run and results
- risks/open questions
```

### B) ASR Bugfix

```text
You are the parent orchestrator for kiku.

Spawn asr_agent and reviewer.

Task: fix <bug> without changing UI behavior.

Ownership:
- asr_agent: crates/kiku-asr, crates/kiku-audio, models/manifests
- reviewer: read-only

Wait policy: wait for asr_agent completion, then run reviewer.

Output format:
- root cause summary
- exact changed files
- regression tests/checks that prove fix
```

### C) Performance Pass

```text
You are the parent orchestrator for kiku.

Spawn rust_core, asr_agent, and reviewer.

Task: reduce <latency/cpu/memory hotspot>.

Ownership:
- rust_core: controller/session/platform plumbing
- asr_agent: inference/audio paths
- reviewer: read-only perf + regression review

Wait policy:
1) parallel profiling/analysis first
2) parallel implementation only if write scopes do not overlap
3) reviewer validates regressions and test coverage

Output format:
- before/after measurements
- changed files by owner
- any behavior tradeoffs
```

### D) Review-Only Gate

```text
Spawn reviewer in read-only mode.
Review current branch for correctness, security/privacy, and missing tests.
Return findings first with severity and file references.
If no findings, state residual risk and test gaps explicitly.
```

## 5) Reusable Task Template (Kiku)

```text
Goal:
Success criteria:

Agent split:
- rust_core:
- asr_agent:
- ui_tauri:
- reviewer:

Ownership boundaries:
- rust_core paths:
- asr_agent paths:
- ui_tauri paths:

Wait policy:

Validation commands:
- cargo check
- cargo test -p kiku-core
- pnpm ui:build

Output format:
- changed files by owner
- tests/checks and results
- findings/risks/open questions
```

## 6) Operational Rules for This Repo

- Keep at most two writer agents active at once.
- Always include one read-only reviewer before final handoff.
- Do not let `ui_tauri` and `asr_agent` write the same files in one run.
- Escalate shared interface changes to the parent before execution.
- Prefer small increments over one large multi-agent batch.

## 7) When to Use Single-Agent Instead

Use single-agent mode when:

- change is contained to one file or one crate,
- no real parallelization opportunity exists,
- the fix is urgent and coordination overhead would dominate.

## References

- https://developers.openai.com/codex/subagents
- https://developers.openai.com/codex/config-reference
- https://developers.openai.com/codex/cli/slash-commands

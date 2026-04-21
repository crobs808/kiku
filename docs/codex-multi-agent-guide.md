# Codex Multi-Agent Guide for VS Code (Any Repo)

Last validated: April 15, 2026 (`codex-cli 0.119.0-alpha.28`)

## What Multi-Agent Means in Codex

- You usually talk to one parent agent.
- The parent agent can spawn specialist subagents when you explicitly ask.
- Subagents work in parallel, then report back to the parent.
- You intervene only for decisions, priorities, and acceptance.

Current platform reality:

- Multi-agent is enabled by feature flag `multi_agent` in current Codex releases.
- Subagent visibility is currently surfaced in Codex App and CLI.
- IDE extension slash commands currently include `/status` and `/review`, but not a team/thread switcher.
- `/team` is not a current built-in slash command in Codex IDE or CLI docs.

## 1) Quick Setup in Any Repo

Run from repo root in the VS Code integrated terminal:

```bash
codex --version
codex --help
codex features list | rg multi_agent
```

Expected:

- You see a Codex version.
- `multi_agent` is listed (typically `stable` and `true`).

Start an interactive session:

```bash
codex
```

## 2) Optional Repo-Level Defaults

Set safe starter limits for subagent fanout:

```bash
mkdir -p .codex/agents
cat > .codex/config.toml <<'EOF'
[agents]
max_threads = 6
max_depth = 1
EOF
```

Why these defaults:

- `max_threads = 6` is enough for parallel work without chaos.
- `max_depth = 1` prevents recursive spawning while you are learning.

## 3) Optional Custom Agent Profiles

Codex supports custom agents via `.codex/agents/*.toml`.

Minimal reviewer:

```bash
cat > .codex/agents/reviewer.toml <<'EOF'
name = "reviewer"
description = "PR reviewer focused on correctness, security, and missing tests."
model = "gpt-5.4"
model_reasoning_effort = "high"
sandbox_mode = "read-only"
developer_instructions = """
Review code like an owner.
Prioritize correctness, security, behavior regressions, and missing test coverage.
Lead with concrete findings and include file references.
Do not make code changes.
"""
EOF
```

Minimal implementation worker:

```bash
cat > .codex/agents/impl_worker.toml <<'EOF'
name = "impl_worker"
description = "Implementation-focused coding agent for scoped, testable changes."
model = "gpt-5.4"
model_reasoning_effort = "medium"
sandbox_mode = "workspace-write"
developer_instructions = """
Own only the files explicitly assigned by the parent.
Make the smallest defensible change.
Run targeted validation for changed behavior.
"""
EOF
```

## 4) Reusable Prompt Cookbook

Copy/paste and replace placeholders.

### A) Parallel Discovery

```text
You are the parent orchestrator.

Spawn 3 read-only subagents in parallel:
1) repo_mapper: map architecture and code paths for <goal>.
2) risk_finder: identify correctness, security, and regression risks.
3) test_planner: propose minimal tests and checks for safe delivery.

Wait policy: wait for all three agents, then merge results.

Output format:
1) Key findings with file references
2) Top 3 implementation steps
3) Must-pass test/check list
```

### B) Split Implementation with Disjoint Ownership

```text
You are the parent orchestrator.

Spawn backend_impl, frontend_impl, and reviewer.

Ownership boundaries (no overlap):
- backend_impl: <backend paths only>
- frontend_impl: <frontend paths only>
- reviewer: read-only across whole repo

Wait policy:
1) Run backend_impl and frontend_impl in parallel.
2) Wait for both writers to finish.
3) Run reviewer on combined diff.
4) Return one consolidated summary.

Output format:
- Files changed by each writer
- Validation commands executed and results
- Risks, follow-ups, and open questions
```

### C) Reviewer-Only Pass

```text
Spawn two read-only reviewers in parallel:
1) correctness_reviewer
2) security_reviewer

Wait policy: wait for both before replying.

Output format:
- Findings first, sorted by severity
- Repro steps or failure scenarios
- Missing tests and exact recommended additions
```

## 5) VS Code Workflow That Works Today

1. Use the Codex extension chat to define the task and success criteria.
2. For heavy multi-agent orchestration, run `codex` in the integrated terminal.
3. Ask the parent to spawn specialists with explicit ownership.
4. Use CLI slash commands to monitor and steer:
   - `/status` for model/context/session status
   - `/agent` to switch active agent thread
   - `/permissions` to adjust approval behavior
   - `/diff` and `/review` before finalizing
5. Merge through the parent summary, then run your repo checks.

## 6) Guardrails

- Keep first run to 2-3 subagents plus one read-only reviewer.
- Assign one owner per writable path.
- Require explicit wait policy in every orchestration prompt.
- Require consolidated parent output with file refs and test outcomes.
- Keep reviewer `sandbox_mode = "read-only"`.

## 7) Anti-Patterns to Avoid

- Overlapping writable ownership across subagents.
- Vague tasks like "clean this up" with no acceptance criteria.
- Deep recursive fanout before baseline process is stable.
- Skipping reviewer pass on cross-cutting changes.
- Treating subagent output as final without parent synthesis.

## 8) Troubleshooting

### "I tried `/team` and it failed."

- Use explicit prompt instructions to spawn subagents.
- Use `/agent` in CLI to switch between threads.

### "I got merge conflicts between agents."

- Stop and re-run with stricter path ownership.
- Keep one writer per subsystem and reviewer as read-only.

### "Context got too noisy."

- Ask for a concise parent summary with only decisions and diffs.
- Start a fresh thread using that summary when needed.

### "This task is slower with multi-agent."

- Use single-agent for small, linear tasks.
- Use multi-agent only when work can be split into truly parallel slices.

## 9) Daily Operating Checklist

```text
[ ] Goal and acceptance criteria are explicit.
[ ] Agent split has disjoint write scopes.
[ ] Wait policy is explicit.
[ ] Reviewer runs read-only.
[ ] Parent returns merged summary with file refs and checks.
[ ] Final diff reviewed before commit.
```

## References

- https://developers.openai.com/codex/subagents
- https://developers.openai.com/codex/config-reference
- https://developers.openai.com/codex/cli/slash-commands
- https://developers.openai.com/codex/ide/slash-commands

# Rust Agentic Guidance: Kiku Adoption Notes

This document translates the imported `rust-agentic-guidance.md` into what should be applied in Kiku now.

## Adopt Now (High Value, Low Churn)

1. Keep changes small and incremental.
2. Run quality gates after each meaningful coding step (`pnpm check`, or `pnpm check:full` before larger merges).
3. Prefer explicit, strongly typed Rust APIs and avoid ambiguous callsites (`foo(false)`, `bar(None)` patterns).
4. Keep crate boundaries clear and isolate platform-specific code behind interfaces/traits.
5. Update docs when APIs or behavior change.
6. Prefer exhaustive `match` handling when practical.

## Kiku-Equivalent Replacements

- `just fmt` -> `cargo fmt --all`
- `just fix` -> `cargo clippy --workspace --all-targets -- -D warnings` plus targeted code changes
- Project-specific test command guidance -> `cargo test -p <crate>` for focused checks, then `pnpm check:full` for broader verification

## Not Directly Applicable To Kiku

The imported guidance includes codex-rs-specific rules and tooling that do not map directly to this repository:

- `codex-*` crate naming and `codex-core` anti-bloat guidance
- Bazel lock/schema workflows (`bazel-lock-update`, `bazel-lock-check`, `BUILD.bazel`)
- `just argument-comment-lint`
- codex-rs app-server and TUI-specific conventions

Keep these as reference context only unless Kiku intentionally adopts equivalent tooling later.

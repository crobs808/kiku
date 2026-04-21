# Development Quality Gates

This project now has explicit local quality gate commands that should run after each meaningful coding step.

## Commands

Run from repo root:

```bash
pnpm check
```

Quick gate (`pnpm check` / `pnpm check:quick`) includes:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `pnpm --dir apps/desktop/ui exec tsc --noEmit`

Full gate (`pnpm check:full`) includes quick gate plus:

1. `cargo test --workspace`
2. `pnpm --dir apps/desktop/ui build`

## Automatic Local Enforcement

Enable the tracked pre-commit hook:

```bash
pnpm hooks:install
```

Once installed, `.githooks/pre-commit` runs `pnpm check:quick` on each commit.

## Notes

- If checks fail, fix the failures before continuing feature work.
- `cargo clippy` currently checks the full workspace, including desktop/Tauri Rust code.
- In environments with restrictive sandboxes, Rust build scripts may require elevated permissions.
- For imported external coding guidance, see `docs/guidance/rust-agentic-adoption-for-kiku.md`.

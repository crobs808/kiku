# Dev Scripts

## Quality Gates

- `./scripts/dev/check.sh --quick`
  - Runs `cargo fmt --check`, `cargo clippy -D warnings`, and UI TypeScript checks.
- `./scripts/dev/check.sh --full`
  - Runs quick checks, then `cargo test --workspace` and `pnpm --dir apps/desktop/ui build`.

Equivalent pnpm shortcuts from repo root:

- `pnpm check` (alias for quick checks)
- `pnpm check:quick`
- `pnpm check:full`

## Git Hook Installation

Install tracked hooks so quality checks run automatically before each commit:

```bash
pnpm hooks:install
```

This sets `core.hooksPath=.githooks` and enables `.githooks/pre-commit`.

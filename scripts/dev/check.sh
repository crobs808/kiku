#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/dev/check.sh --quick
  ./scripts/dev/check.sh --full

Modes:
  --quick  Run formatting/lint/type checks (default).
  --full   Run quick checks plus workspace tests and UI production build.
EOF
}

MODE="${1:---quick}"
if [[ "$MODE" != "--quick" && "$MODE" != "--full" ]]; then
  usage
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "[2/3] cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

echo "[3/3] pnpm --dir apps/desktop/ui exec tsc --noEmit"
pnpm --dir apps/desktop/ui exec tsc --noEmit

if [[ "$MODE" == "--full" ]]; then
  echo "[4/5] cargo test --workspace"
  cargo test --workspace

  echo "[5/5] pnpm --dir apps/desktop/ui build"
  pnpm --dir apps/desktop/ui build
fi

echo "Checks completed successfully."

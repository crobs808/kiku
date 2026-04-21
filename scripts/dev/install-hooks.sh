#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

chmod +x .githooks/pre-commit scripts/dev/check.sh scripts/dev/install-hooks.sh
git config core.hooksPath .githooks

echo "Installed Git hooks path: .githooks"
echo "Pre-commit will now run: pnpm check:quick"

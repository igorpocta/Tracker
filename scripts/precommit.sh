#!/usr/bin/env bash
#
# Local pre-push gate (Phase 18C — Item 25).
#
# Run all the checks we want CI to also enforce, in roughly the order they're
# likely to fail. The script bails on the first failure (`set -e`) so the
# developer sees the actionable error immediately.
#
# Manual invocation: `./scripts/precommit.sh`. The git hook installed by
# `./scripts/install-hooks.sh` exec's this file on `git push`, so pushing to
# origin also runs the gate. (`release.sh` runs it too, before tag + build.)
set -euo pipefail

# Always run from the repo root, regardless of CWD.
cd "$(dirname "$0")/.."

# Pretty progress line. Avoid emojis (the repo style guide bans them in
# committed code).
step() {
  printf '\n→ %s\n' "$*"
}

step "TypeScript typecheck"
npm run typecheck

step "ESLint"
npm run lint

step "Vitest"
npm run test

step "Frontend build"
npm run build

step "Rust format check"
( cd src-tauri && cargo fmt -- --check )

step "Rust clippy"
( cd src-tauri && cargo clippy --all-targets -- -D warnings )

step "Rust tests"
( cd src-tauri && cargo test --no-fail-fast )

printf '\n✓ All checks passed\n'

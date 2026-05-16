#!/usr/bin/env bash
#
# Vydání nové verze Trackeru.
#
# Použití:
#   ./scripts/release.sh              # interaktivně se zeptá na verzi
#   ./scripts/release.sh 0.2.0        # explicitní verze
#   ./scripts/release.sh patch        # 0.1.0 → 0.1.1
#   ./scripts/release.sh minor        # 0.1.0 → 0.2.0
#   ./scripts/release.sh major        # 0.1.0 → 1.0.0
#   ./scripts/release.sh 0.2.0-rc.1   # pre-release (workflow označí Release jako pre)
#
# Skript:
#   1. Ověří že jsi na čisté main branch synchronizované s origin.
#   2. Bumpne verzi v package.json + tauri.conf.json + Cargo.toml přes
#      `npm run version:bump`.
#   3. Spustí precommit gate (typecheck, lint, vitest, frontend build,
#      cargo fmt, clippy, cargo test).
#   4. Commitne, otaguje (`vX.Y.Z`), pushne branch i tag.
#   5. GitHub Actions release workflow (`.github/workflows/release.yml`)
#      poté nabuilduje DMG (mac arm64) + MSI (windows x64) a publikuje
#      GitHub Release. Sentry sourcemap upload se dělá jen pokud má repo
#      nastavené secrety; pokud ne, build proběhne bez něj a workflow
#      ten krok přeskočí (ekvivalent SKIP_SENTRY=1).
#
# Pro lokální build (mimo CI) je k dispozici `npm run build:local`, který
# spouští `scripts/build-release.sh` se Sentry integrací (a stále respektuje
# SKIP_SENTRY=1).

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}▶${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}⚠${NC} $*"; }
err()   { echo -e "${RED}✗${NC} $*" >&2; }

ask_yes_no() {
  local prompt="$1"
  local answer
  read -r -p "$prompt (y/N) " answer
  [[ "$answer" =~ ^[Yy]$ ]]
}

revert_bump() {
  warn "Reverting bumpnuté soubory..."
  git checkout -- package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml 2>/dev/null || true
}

# 1. Přesun do repo root
cd "$(git rev-parse --show-toplevel)"

echo
echo -e "${BOLD}═══ Tracker — Release ═══${NC}"
echo

# 2. Branch check
branch=$(git branch --show-current)
if [ "$branch" != "main" ]; then
  warn "Nejsi na main (aktuální branch: $branch)."
  ask_yes_no "Opravdu pokračovat?" || { err "Zrušeno."; exit 1; }
fi

# 3. Čistý working tree
if [ -n "$(git status --porcelain)" ]; then
  err "Working tree není čistý — commitni nebo stashni změny nejdřív:"
  git status --short
  exit 1
fi

# 4. Sync s origin
info "Fetchuju origin..."
git fetch origin --tags --quiet

local_sha=$(git rev-parse HEAD)
remote_sha=$(git rev-parse "origin/$branch" 2>/dev/null || echo "")
if [ -n "$remote_sha" ] && [ "$local_sha" != "$remote_sha" ]; then
  warn "Local $branch ($local_sha) není v sync s origin ($remote_sha)."
  ask_yes_no "Opravdu pokračovat?" || { err "Zrušeno. Spusť 'git pull' nejdřív."; exit 1; }
fi

# 5. Aktuální verze
current=$(node -p "require('./package.json').version")
echo
info "Aktuální verze: ${BOLD}${GREEN}$current${NC}"

# 6. Získat novou verzi
if [ "$#" -ge 1 ]; then
  input="$1"
else
  echo
  echo "Zadej novou verzi:"
  echo "  patch / minor / major / X.Y.Z[-tag]"
  read -r -p "> " input
fi

if [ -z "$input" ]; then
  err "Verze nezadána."
  exit 1
fi

# 7. Bump
info "Bumpuju verzi..."
npm run version:bump -- "$input"

new=$(node -p "require('./package.json').version")

if [ "$new" = "$current" ]; then
  err "Verze se nezměnila."
  exit 1
fi

echo
info "Změna: ${BOLD}${GREEN}$current → $new${NC}"
echo
git diff --stat
echo

# 8. Tag check
tag="v$new"
if git rev-parse "$tag" >/dev/null 2>&1; then
  err "Tag $tag už existuje lokálně. Zruš ho přes 'git tag -d $tag' nebo zvol jinou verzi."
  revert_bump
  exit 1
fi
if git ls-remote --tags origin "refs/tags/$tag" | grep -q "$tag"; then
  err "Tag $tag už existuje na origin. Zruš ho přes 'git push --delete origin $tag' nebo zvol jinou verzi."
  revert_bump
  exit 1
fi

# 9. Detekce pre-release
is_prerelease="false"
if [[ "$new" == *-* ]]; then
  is_prerelease="true"
fi

# 10. Plán
echo "Bude provedeno:"
echo "  • Precommit gate (typecheck, lint, vitest, build, fmt, clippy, cargo test)"
echo "  • git commit -am \"chore(release): $tag\""
echo "  • git tag $tag"
echo "  • git push origin $branch"
echo "  • git push origin $tag"
[ "$is_prerelease" = "true" ] && echo "  • GitHub Release bude označen jako pre-release (kvůli pomlčce v tagu)"
echo

ask_yes_no "Pokračovat?" || { revert_bump; err "Zrušeno."; exit 1; }

# 11. Gate
info "Spouštím precommit gate..."
if ! ./scripts/precommit.sh >/tmp/release-gate.log 2>&1; then
  err "Precommit gate selhal. Posledních 40 řádků:"
  tail -40 /tmp/release-gate.log
  revert_bump
  exit 1
fi
ok "Gate prošla."

# 12. Commit
info "Commit..."
git commit -am "chore(release): $tag"

# 13. Tag
info "Tag $tag..."
git tag "$tag"

# 14. Push
info "Push $branch..."
git push origin "$branch"
info "Push $tag..."
git push origin "$tag"

# 15. Done — odkazy
echo
ok "Release $tag pushnutý!"
echo

remote_url=$(git config --get remote.origin.url \
  | sed -e 's|git@github.com:|https://github.com/|' \
        -e 's|\.git$||')

info "Sleduj build:"
echo "  ${BLUE}$remote_url/actions${NC}"
echo
info "Až workflow doběhne, release najdeš na:"
echo "  ${BLUE}$remote_url/releases/tag/$tag${NC}"
echo

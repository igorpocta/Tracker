#!/usr/bin/env bash
#
# Release build s integrací Sentry source maps.
#
# Co tento skript dělá:
#   1. Validuje, že máme nastavené potřebné env vary:
#        - TRACKER_SENTRY_DSN_BACKEND, TRACKER_SENTRY_DSN_FRONTEND
#          (volitelně — pokud chybí, sentry-cli upload se přeskočí, build
#          stále projde a Sentry zůstane opt-in vypnutý).
#        - SENTRY_AUTH_TOKEN, SENTRY_ORG, SENTRY_PROJECT_BACKEND,
#          SENTRY_PROJECT_FRONTEND — vyžadováno pro sourcemaps upload.
#   2. Sestaví release tag z git SHA + npm verze.
#   3. Frontend build (vite) s baked-in DSN.
#   4. Tauri build (Rust) s baked-in DSN.
#   5. Nahraje sourcemaps + Rust debug info do Sentry, finalizuje release.
#
# Pokud nemáš Sentry token, spusť `SKIP_SENTRY=1 ./scripts/build-release.sh`
# a kroky 1 a 5 se přeskočí.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ---------- Release identifier --------------------------------------------
PKG_VERSION="$(node -p "require('./package.json').version")"
GIT_SHA="$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
RELEASE="tracker@${PKG_VERSION}+${GIT_SHA}"
export VITE_RELEASE="$RELEASE"
echo "▶ Release tag: $RELEASE"

# ---------- Sentry env prep ------------------------------------------------
SKIP_SENTRY="${SKIP_SENTRY:-0}"
if [[ "$SKIP_SENTRY" != "1" ]]; then
  : "${SENTRY_AUTH_TOKEN:?nastavte SENTRY_AUTH_TOKEN nebo SKIP_SENTRY=1}"
  : "${SENTRY_ORG:?nastavte SENTRY_ORG nebo SKIP_SENTRY=1}"
  : "${SENTRY_PROJECT_FRONTEND:?nastavte SENTRY_PROJECT_FRONTEND nebo SKIP_SENTRY=1}"
  : "${SENTRY_PROJECT_BACKEND:?nastavte SENTRY_PROJECT_BACKEND nebo SKIP_SENTRY=1}"
  if ! command -v sentry-cli >/dev/null 2>&1; then
    echo "✗ sentry-cli není v PATH. Nainstaluj: brew install getsentry/tools/sentry-cli"
    exit 1
  fi
  export SENTRY_LOG_LEVEL="${SENTRY_LOG_LEVEL:-info}"
fi

# ---------- Frontend build (vite produces sourcemaps) ----------------------
echo "▶ Frontend build…"
# Sourcemaps musí být enabled v vite.config.ts (`build.sourcemap: true`),
# jinak Sentry nemá co nahrát.
npm run build

# ---------- Tauri build (Rust → binary, baked DSN) -------------------------
echo "▶ Tauri build…"
# DSN se zapeče přes option_env! v sentry_init.rs.
npm run tauri build

# ---------- Sentry release + sourcemaps upload -----------------------------
if [[ "$SKIP_SENTRY" == "1" ]]; then
  echo "↪ SKIP_SENTRY=1, Sentry upload přeskočen."
  exit 0
fi

echo "▶ Sentry release: $RELEASE"
sentry-cli releases new "$RELEASE" \
  --project "$SENTRY_PROJECT_FRONTEND" \
  --project "$SENTRY_PROJECT_BACKEND"

# Frontend sourcemaps (vite output).
sentry-cli sourcemaps upload \
  --project "$SENTRY_PROJECT_FRONTEND" \
  --release "$RELEASE" \
  ./dist

# Rust debug info (symbols) — sentry-cli ho najde v target/release.
# Pokud Rust release nebyl s `debug = true`, nepřinese to nic užitečného,
# tedy se obejde s prázdným uploadem.
sentry-cli debug-files upload \
  --project "$SENTRY_PROJECT_BACKEND" \
  --include-sources \
  ./src-tauri/target/release || echo "↪ Žádné Rust debug-files nenalezeny (build bez debug=true)."

sentry-cli releases set-commits "$RELEASE" --auto || true
sentry-cli releases finalize "$RELEASE" \
  --project "$SENTRY_PROJECT_FRONTEND" \
  --project "$SENTRY_PROJECT_BACKEND"

echo "✓ Hotovo. Release: $RELEASE"

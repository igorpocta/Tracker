# Tracker Bridge — rozšíření prohlížeče

Nahlásí desktopové aplikaci **Tracker**, na kterém Jira úkolu právě jsi, a umožní
z prohlížeče spustit/zastavit časovač. Mluví s lokálním bridge serverem Trackeru
na `http://127.0.0.1:27420` (jen loopback, chráněno bearer tokenem).

Funguje v **Chrome** i **Firefox** (Manifest V3).

## Instalace (unpacked / dočasně)

**Chrome / Edge / Brave**
1. `chrome://extensions`
2. zapni **Developer mode** (vpravo nahoře)
3. **Load unpacked** → vyber složku `browser-extension/`

**Firefox** (121+)
1. `about:debugging#/runtime/this-firefox`
2. **Load Temporary Add-on…** → vyber `browser-extension/manifest.json`
   (dočasné add-ony se po restartu odeberou — pro trvalé je potřeba podepsaný balík)

## Nastavení tokenu

Bridge je chráněný per-install tokenem, aby na něj nedosáhla libovolná webová
stránka. Token najdeš v datovém adresáři Trackeru v souboru
`browser-bridge-token`:

- **macOS:** `~/Library/Application Support/com.tracker.app/browser-bridge-token`
- **Windows:** `%APPDATA%\com.tracker.app\browser-bridge-token`
- **Linux:** `~/.local/share/com.tracker.app/browser-bridge-token`

Zkopíruj obsah, otevři popup rozšíření → **Nastavení** → vlož token → **Uložit token**.

## Použití

1. Spusť desktopový Tracker (musí běžet — bridge poslouchá jen když aplikace běží).
2. Otevři Jira úkol v prohlížeči (`https://*.atlassian.net/…`).
3. Klikni na ikonu rozšíření:
   - stav **připojeno** = bridge běží a token sedí,
   - zobrazí se detekovaný úkol,
   - **▶ Spustit** / **■ Zastavit** ovládá časovač Trackeru.

Content script navíc průběžně hlásí „na co se právě díváš", takže Tracker může
nabízet chytré návrhy worklogů.

## Poznámky / omezení

- **Jen Jira Cloud** (`*.atlassian.net`). Self-hosted / vlastní host by chtěl
  přidat do `host_permissions` + `content_scripts.matches` v `manifest.json`.
- Endpointy bridge: `GET /status`, `GET /timer-state`, `GET|POST /visible-ticket`,
  `POST /start-timer`, `POST /stop-timer` (viz `src-tauri/src/server.rs`).
- Token se ukládá do `chrome.storage.local` rozšíření, nikam jinam se neposílá.

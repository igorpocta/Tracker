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

## Self-hosted / on-premise Jira

Rozšíření standardně běží jen na `*.atlassian.net`. Pro vlastní Jiru otevři popup
→ **Nastavení** → pole **Self-hosted Jira**, vlož adresu (`https://jira.firma.cz`)
a klikni **Povolit tento host**. Prohlížeč se zeptá na povolení pro tuto doménu
a detekce úkolů se na ní zapne (dynamická registrace content-scriptu, přežije
restart). Přidané hosty lze v popupu zase odebrat.

## Focus mode — blokování webů

Rozšíření zároveň vynucuje pravidla **Focus mode**. Blokovanou stránku
přesměruje na `http://127.0.0.1:27420/blocked`, kde Tracker ukáže odpočet a
dlaždice s povolenými weby.

1. V Trackeru otevři **Nastavení → Focus** a přidej blokované (nebo povolené)
   domény.
2. V popupu rozšíření klikni na **Povolit blokování webů**. Prohlížeč se zeptá
   na přístup ke všem stránkám — bez něj přesměrování fungovat nemůže.
3. Focus spusť v Trackeru (boční panel nebo popover u hodin). Pravidla se
   propíšou do prohlížeče do jedné sekundy.

Blokování zajišťuje `declarativeNetRequest`, takže funguje i když je service
worker uspaný.

**Když Tracker neběží, rozšíření všechna pravidla smaže.** Jinak by vypnutá
aplikace nechala prohlížeč přesměrovávat na adresu, která už neodpovídá.
Rozšíření se proto pravidelně ptá, jestli je bridge naživu, a po dvou
neúspěších se vypne. Focus mode je nástroj sebekázně, ne zámek.

Safari extension neexistuje — na macOS ho Tracker obchází přes AppleScript a
přepisuje URL aktivního tabu přímo. Stejná záloha funguje i pro Chrome na
macOS, kdyby rozšíření nebylo nainstalované.

## Poznámky / omezení

- Standardní matches: `*.atlassian.net`; další hosty se přidávají za běhu (viz výše).
- Endpointy bridge: `GET /status`, `GET /timer-state`, `GET|POST /visible-ticket`,
  `POST /start-timer`, `POST /stop-timer`, `GET /focus/state` (viz
  `src-tauri/src/server.rs`). Blokovací stránka `GET /blocked` a `GET /focus/ping`
  token nevyžadují — volá je prohlížeč, kterému token předat nelze.
- Token se ukládá do `chrome.storage.local` rozšíření, nikam jinam se neposílá.

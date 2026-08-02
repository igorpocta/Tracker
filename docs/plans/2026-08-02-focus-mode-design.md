# Focus mode — design

Datum: 2026-08-02

Focus mode blokuje rozptylující aplikace a weby po dobu soustředěné práce.
Zapíná a vypíná se z bočního panelu nebo z popoveru u hodin; pravidla se
nastavují v Nastavení → Focus.

## Rozhodnutí

| Oblast | Volba |
|---|---|
| Nechtěná aplikace | Skrýt + upozornění. `kill` jen pro aplikace, kde ho uživatel výslovně zapne. |
| Safari | AppleScript polling (`osascript`), ne Safari Web Extension. |
| Ostatní prohlížeče | MV3 extension s `declarativeNetRequest`; na macOS navíc AppleScript jako záloha. |
| Notifikace | macOS Shortcuts CLI, Windows deep-link do systémového nastavení. |
| Konfigurace | Jeden profil, blocklist i allowlist (přísný režim). |

## Bezpečnostní hranice

Engine sahá **jen na aplikaci, kterou uživatel právě přepnul do popředí**.
Procesy na pozadí neprochází, takže přísný režim nemůže shodit systém.
Jedinou výjimkou je jednorázový průchod při startu Focusu, který ukončí
aplikace s explicitním pravidlem `action = kill`; v přísném režimu se
nespouští nikdy.

Nad tím stojí safe-list, kterého se enforcement nikdy nedotkne:

- macOS: `com.apple.finder`, `com.apple.dock`, `com.apple.systemuiserver`,
  `com.apple.controlcenter`, `com.apple.notificationcenterui`,
  `com.apple.loginwindow`, `com.apple.WindowManager`, `com.tracker.app`
- Windows: `explorer.exe`, `dwm.exe`, `winlogon.exe`, `csrss.exe`,
  `lsass.exe`, `services.exe`, `sihost.exe`, `ctfmon.exe`, `taskmgr.exe`,
  `tracker.exe`

Přísný režim navíc nikdy nepoužije `kill`, vždy jen `hide`.

Focus mode není bezpečnostní produkt. Uživatel může Tracker ukončit nebo
rozšíření vypnout. Je to nástroj sebekázně.

## Datový model

Migrace `0018_focus_mode.sql`:

```sql
CREATE TABLE focus_rules (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT    NOT NULL CHECK (kind IN ('app', 'site')),
    mode       TEXT    NOT NULL CHECK (mode IN ('block', 'allow')),
    pattern    TEXT    NOT NULL,
    label      TEXT,
    action     TEXT    NOT NULL DEFAULT 'hide' CHECK (action IN ('hide', 'kill')),
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);
```

Zbytek jde do `app_settings`: `focus_strict_apps`, `focus_strict_sites`,
`focus_block_notifications`, `focus_shortcut_on`, `focus_shortcut_off`,
`focus_default_duration_min`, `focus_active_until`.

## Vyhodnocování pravidel

`src-tauri/src/focus/rules.rs`, čistá funkce bez I/O.

Aplikace — vzor se porovnává case-insensitive proti bundle ID, názvu
spustitelného souboru i zobrazovanému jménu, takže jedno pravidlo funguje na
obou platformách:

1. safe-list → povoleno
2. shoda s `allow` pravidlem → povoleno
3. shoda s `block` pravidlem → blokováno zvolenou akcí
4. přísný režim → blokováno (`hide`), jinak povoleno

Weby — o záběru rozhoduje to, co uživatel napsal. Registrovatelná doména
(`seznam.cz`) znamená celý web včetně subdomén; konkrétní host
(`www.seznam.cz`) platí jen pro něj. `*.host` vynutí subdomény i tam, kde by
je host sám nedostal. Dvouúrovňové sufixy (`example.co.uk`) řeší krátký
vestavěný seznam, ne plný Public Suffix List:

1. loopback (`localhost`, `127.0.0.1`, `[::1]`) → povoleno
2. shoda s `allow` pravidlem → povoleno
3. shoda s `block` pravidlem → blokováno
4. přísný režim → blokováno, jinak povoleno

`allow` má přednost před `block`, takže výjimka z širokého blokovacího
pravidla se dá zapsat přímo.

## Blokování webů

Rozšíření drží dynamická `declarativeNetRequest` pravidla. Vyhodnocuje je
prohlížeč nativně, takže fungují i když service worker spí. Blokovaná adresa
se přes `regexSubstitution` přesměruje na
`http://127.0.0.1:27420/blocked?u=<původní adresa>`.

Priority pravidel: loopback allow (100) > allow pravidla (3) > block pravidla
(2) > catch-all přísného režimu (1).

Synchronizace: long-poll `GET /focus/state?wait=25&gen=N` dá změnu pod sekundu,
`chrome.alarms` à 1 minutu funguje jako záchrana, když worker umře.

**Failsafe:** po dvou selháních v řadě nebo při `runtime.onStartup` rozšíření
smaže všechna pravidla. Bez toho by vypnutý Tracker znamenal prohlížeč
přesměrovávající na mrtvou adresu.

Safari a macOS Chrome jedou přes `osascript`, ale jen když je ten prohlížeč
v popředí — jinak nulová zátěž. Před spuštěním skriptu se přes `NSWorkspace`
ověří, že prohlížeč běží, jinak by ho AppleScript sám nastartoval.

## Blokovací stránka

`GET /blocked?u=…` je server-rendered a bez autentizace. Ukazuje nadpis
„Focus mode je aktivní", blokovaný host, odpočet do konce a dlaždice
s povolenými weby. Server-rendering místo veřejného JSON API znamená, že
si žádná lokální stránka seznam povolených domén nepřečte.

`GET /focus/ping` vrací jen `{"active": bool}`. Stránka ho polluje a po
skončení Focusu se sama vrátí na původní adresu.

## Notifikace

macOS nemá veřejné API pro zapnutí Focusu, a zásahy do
`~/Library/DoNotDisturb/DB` Apple opakovaně rozbil. Spouštíme proto
uživatelovu zkratku přes `shortcuts run`; v nastavení se vybírá ze seznamu
vrácného `shortcuts list`.

Windows Focus Assist rovněž veřejné API nemá, takže jen otevřeme
`ms-settings:quiethours`. Nedokumentované registry zápisy nepoužíváme.

Vlastní notifikace Trackeru se během Focusu tlumí vždy.

## macOS podpis

`Entitlements.plist` potřebuje `com.apple.security.automation.apple-events`
a `Info.plist` klíč `NSAppleEventsUsageDescription`. Bez obojího AppleScript
pod hardened runtime spadne. Systémový dotaz na povolení se váže na podpis,
takže nepodepsané buildy se ptají po každé aktualizaci.

## Rozhraní

- **Nastavení → Focus** — pravidla pro aplikace i weby, přepínače přísného
  režimu, notifikace, stav rozšíření
- **Boční panel** — ikona štítu nad ozubeným kolem, klik spustí/zastaví
- **Popover u hodin** — stejné tlačítko a zbývající čas
- **Overlay** — malé okno bez interakce, které samo zmizí

## Testy

Rust pokrývá vyhodnocování pravidel včetně subdomén, přednosti `allow` a
nedotknutelnosti safe-listu. TypeScript pokrývá normalizaci vzorů a přepínač
v bočním panelu. Generátor pravidel rozšíření je čistý modul testovaný
vitestem.

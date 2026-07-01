# Tracker

Desktopový time tracker pro **Jira Cloud** a **Freelo**. Běží lokálně, bez
přihlašování a bez cloudové synchronizace mimo poskytovatele, které si sami
zvolíte.

**Platformy:** macOS · Windows &nbsp;•&nbsp; **Postaveno na:** Tauri 2, React, TypeScript, SQLite

## Funkce

- **Sledování času jedním kliknutím** — z tray ikony, popoveru i hlavního okna.
- **Obousměrná synchronizace worklogů** s Jira Cloud a Freelo, včetně řešení
  konfliktů.
- **Více účtů** — několik Jira/Freelo připojení vedle sebe, korektně oddělených
  i při shodných klíčích úkolů.
- **Motivace a přehled** — denní cíl, streaks, kalendář pracovních dnů.
- **Reporty** s grafy a exportem do XLSX.
- **Idle detekce** ve stylu Toggl (Keep / Discard / Discard & Continue).
- **Automatické aktualizace** — podepsané, s potvrzením uživatele.
- **České rozhraní** a dynamická ikona podle zvolené barevné palety.

## Instalace

Stáhněte balíček z [Releases](https://github.com/igorpocta/Tracker/releases)
a nainstalujte:

- **macOS** — `.dmg`
- **Windows** — `.msi`

Po prvním spuštění:

1. Otevřete **Nastavení → Připojení** a přidejte účet Jira nebo Freelo.
2. Volitelně vyplňte hodinovou sazbu v **Nastavení → Reporting**.
3. Hotovo — můžete začít stopovat čas.

## Aktualizace

Tracker se aktualizuje automaticky z GitHub Releases. Balíčky jsou podepsané a
ověřují se proti veřejnému klíči zabudovanému v aplikaci. Kontrola proběhne
krátce po spuštění a nejvýše jednou denně; ručně kdykoli v
**Nastavení → O aplikaci**. Nová verze se nainstaluje až po vašem potvrzení —
nikdy tiše a nikdy nevynucuje restart během běžící časomíry.

## Data a soukromí

- Všechna data zůstávají lokálně v **SQLite** databázi ve vašem aplikačním
  adresáři — žádný účet, žádná cloudová synchronizace mimo zvolené poskytovatele.
- Přístupové tokeny se ukládají v souboru `secret.toml` s restriktivními
  oprávněními (`chmod 0600` na Unixu). Databáze ani tokeny nejsou na disku
  šifrované.
- Žádná telemetrie kromě **volitelného** anonymního reportu chyb (opt-in).

## Licence

[MIT](LICENSE) — Igor Počta, 2026.

---

Vyvíjíte nebo sestavujete Tracker ze zdrojů? Viz [docs/development.md](docs/development.md).

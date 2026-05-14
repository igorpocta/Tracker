# Tracker

Desktopová aplikace pro sledování času s napojením na **Jira Cloud** a **Freelo**.
Lokální data, žádné přihlašování, žádná cloudová synchronizace mimo zvolené
poskytovatele.

- **Platformy:** macOS, Windows
- **Stack:** Tauri 2 + React + TypeScript + SQLite

## Funkce

- Sledování času s jedním kliknutím (tray, popover, hlavní okno).
- Synchronizace worklogů s Jira Cloud a Freelo (obousměrná, řešení konfliktů).
- Pomodoro režim, denní cíl, kalendář pracovních dnů, streaks.
- Reporty s grafy a XLSX exportem.
- Idle detekce (Toggl-style: Keep / Discard / Discard & Continue).
- Dynamická ikona aplikace podle vybrané barevné palety.
- České UI, lokální data v SQLite, vše šifrováno na disku.

## Instalace

Stáhněte vhodný balíček ze [Releases](https://github.com/igorpocta/Tracker/releases)
(`.dmg` pro macOS, `.msi` pro Windows), nainstalujte a spusťte.

Po prvním spuštění:

1. Otevřete **Nastavení → Připojení** a přidejte Jira nebo Freelo účet.
2. Vyplňte hodinovou sazbu v **Nastavení → Reporting** (volitelné).
3. Hotovo — můžete začít stopovat.

## Vývoj

### Předpoklady

- Node.js 22+
- Rust stable s `rustfmt` a `clippy`
- macOS: Xcode command-line tools (`xcode-select --install`)

### První spuštění

```bash
npm install
./scripts/install-hooks.sh     # nainstaluje pre-commit hook
npm run tauri dev              # spustí aplikaci s hot-reloadem
```

### Kontrolní gate

`scripts/precommit.sh` spouští stejné kontroly jako CI:

- `npm run typecheck`, `npm run lint`, `npm run test`, `npm run build`
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`

Manuální spuštění: `./scripts/precommit.sh`

### Produkční build

```bash
cargo tauri build
```

Balíčky najdete v `src-tauri/target/release/bundle/`.

## Licence

[MIT](LICENSE) — Igor Počta, 2026.

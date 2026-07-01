# Vývoj

Interní dokumentace pro sestavení a vývoj Trackeru. Pro instalaci a používání
aplikace viz [README](../README.md).

## Předpoklady

- Node.js 22+
- Rust stable s `rustfmt` a `clippy`
- macOS: Xcode command-line tools (`xcode-select --install`)

## První spuštění

```bash
npm install
./scripts/install-hooks.sh     # nainstaluje git hook (gate běží na pre-push)
npm run tauri dev              # spustí aplikaci s hot-reloadem
```

## Kontrolní gate

`scripts/precommit.sh` spouští stejné kontroly jako CI:

- `npm run typecheck`, `npm run lint`, `npm run test`, `npm run build`
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`

Manuální spuštění: `./scripts/precommit.sh`

## Produkční build

```bash
cargo tauri build
```

Balíčky najdete v `src-tauri/target/release/bundle/`.

## Vydání verze + auto-update

Podrobný one-time setup podpisových klíčů a CI je v
[auto-updater-setup.md](auto-updater-setup.md). Vlastní vydání:

1. Bump verze (`tauri.conf.json` + `package.json`), commit.
2. Tag `vX.Y.Z` a push tagu — workflow sestaví podepsané balíčky, vygeneruje
   `latest.json` a vytvoří GitHub Release.

Tag s pomlčkou (např. `v1.2.0-rc.1`) se publikuje jako pre-release a `latest.json`
endpoint ho uživatelům nenabídne.

# Auto-updater — setup (one-time)

The app ships a Tauri v2 updater that reads a **signed** `latest.json` from
GitHub Releases (`https://github.com/igorpocta/Tracker/releases/latest/download/latest.json`).
Signatures are verified natively against a public key baked into the app, so
security does not rest on trusting a GitHub asset.

The **code, config, capabilities and CI are already wired**. Two things are
yours to do before the first signed release, because they involve a private
signing key that must never touch a chat log or the repo:

## 1. Generate the updater signing key

```sh
# Writes the PRIVATE key to the given path and the PUBLIC key to <path>.pub.
npm run tauri -- signer generate -w ~/.tracker/updater.key
# (you'll be asked for an optional password — recommended)
```

- Copy the **public** key (contents of `~/.tracker/updater.key.pub`, a single
  base64 line) into `src-tauri/tauri.conf.json` →
  `plugins.updater.pubkey`, replacing `REPLACE_WITH_UPDATER_PUBLIC_KEY`.
- **Back up the private key offline** (password manager / secure vault). If you
  lose it you can never again ship an update that existing installs will accept.

## 2. Add the GitHub Actions secrets

Repo → Settings → Secrets and variables → Actions:

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | full contents of `~/.tracker/updater.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password you set (empty string if none) |

Harden it: put these in a **protected GitHub Environment** with required
reviewers, and protect the `v*` tags. Anyone who obtains the private key can
ship a malicious update to every install.

## 3. Ship it

- `createUpdaterArtifacts: true` makes `tauri build` emit the updater bundle
  (`.app.tar.gz` on macOS, installer on Windows) plus a `.sig` per artifact.
- The release job runs `scripts/build-latest-json.mjs`, which assembles
  `latest.json` from the signatures and uploads it alongside the installers.
- Cut a release the usual way: bump the version, push a `vX.Y.Z` tag. A tag
  build **fails on purpose** if the signing secret is missing — a release
  without a valid signature can't self-update.

## Product behaviour

- A silent check runs a few seconds after launch, then at most once per day.
- Settings → O aplikaci has a manual "Zkontrolovat aktualizace" button.
- An available update shows a top banner (Stáhnout → Restartovat a dokončit).
- **Never a silent restart.** While a timer is running the banner says the
  timer will resume after restart and leaves the restart to the user.

## First-release caveat

The first build that *adds* the updater (this one) can't reach existing users
automatically — they must install it once by hand from GitHub Releases. Every
release after that updates in place.

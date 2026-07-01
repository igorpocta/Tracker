# Auto-updater — setup (one-time)

The app ships a Tauri v2 updater that reads a **signed** `latest.json` from
GitHub Releases (`https://github.com/igorpocta/Tracker/releases/latest/download/latest.json`).
Signatures are verified natively against a public key baked into the app, so
security does not rest on trusting a GitHub asset.

The **code, config, capabilities and CI are already wired**, and the signing
**keypair is already generated** (empty password) with the public key baked
into `tauri.conf.json`. What's left is yours, because it involves the private
key, which must never touch a chat log or the repo:

## 1. Signing key — DONE, but back it up

Generated at `~/.tracker/updater.key` (private) + `~/.tracker/updater.key.pub`
(public, already in `tauri.conf.json → plugins.updater.pubkey`). Empty password.

- **Back up `~/.tracker/updater.key` offline now** (password manager / vault).
  If you lose it you can never again ship an update existing installs accept.
- Prefer a passworded key? Regenerate:
  `npm run tauri -- signer generate -w ~/.tracker/updater.key -f`, then paste
  the new `~/.tracker/updater.key.pub` into `tauri.conf.json`.

## 2. Add the GitHub Actions secrets

Copy the private key to the clipboard (it is NOT printed anywhere):

```sh
cat ~/.tracker/updater.key | pbcopy
```

Repo → Settings → Secrets and variables → Actions:

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | paste (full contents of `~/.tracker/updater.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | empty string (the key has no password) |

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

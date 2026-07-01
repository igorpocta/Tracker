#!/usr/bin/env node
/**
 * Build the Tauri updater feed (`latest.json`) from the CI bundle artifacts.
 *
 * Why a custom script: the release pipeline stays custom (Sentry, DMG
 * notarization), so we assemble the updater feed ourselves instead of letting
 * tauri-action do it.
 *
 * Input : a directory of downloaded CI artifacts, one sub-dir per matrix
 *         target — `Tracker-<rust-target>/…` (see build.yml upload steps).
 * Output: `<outDir>/latest.json` plus, for macOS, arch-renamed copies of the
 *         `.app.tar.gz` (+ `.sig`). macOS names its updater tarball
 *         `Tracker.app.tar.gz` regardless of arch, so uploading both targets to
 *         one GitHub Release would collide — we rename to
 *         `Tracker_<version>_<arch>.app.tar.gz`. Windows installers already carry
 *         a unique name, so their `latest.json` URL points at the file uploaded
 *         straight from the artifacts.
 *
 * Usage: node scripts/build-latest-json.mjs <tag> [artifactsDir] [outDir]
 *   tag           e.g. `v1.0.7` (defaults to $GITHUB_REF_NAME)
 *   artifactsDir  defaults to `artifacts`
 *   outDir        defaults to `updater-dist`
 * Env: RELEASE_NOTES (optional) → latest.json `notes`.
 */
import { copyFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

const REPO = "igorpocta/Tracker";

const tag = process.argv[2] || process.env.GITHUB_REF_NAME;
const artifactsDir = process.argv[3] || "artifacts";
const outDir = process.argv[4] || "updater-dist";
if (!tag) {
  console.error("usage: build-latest-json.mjs <tag> [artifactsDir] [outDir]");
  process.exit(1);
}
const version = tag.replace(/^v/, "");

/** Rust target → (platform key for latest.json, macOS arch label or null). */
const TARGETS = {
  "aarch64-apple-darwin": { platform: "darwin-aarch64", macArch: "aarch64" },
  "x86_64-apple-darwin": { platform: "darwin-x86_64", macArch: "x64" },
  "x86_64-pc-windows-msvc": { platform: "windows-x86_64", macArch: null },
};

/** Recursively collect every file path under `dir`. */
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

mkdirSync(outDir, { recursive: true });
const releaseUrl = (name) =>
  `https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(name)}`;

const platforms = {};

for (const [target, { platform, macArch }] of Object.entries(TARGETS)) {
  const dir = join(artifactsDir, `Tracker-${target}`);
  let files;
  try {
    files = walk(dir);
  } catch {
    console.warn(`↪ no artifacts for ${target} (${dir}) — skipping`);
    continue;
  }

  // Locate the signed updater artifact for this target. Windows ships NSIS
  // only (the `-setup.exe`); MSI was dropped so install + auto-update use the
  // same installer type and never produce a duplicate shortcut.
  const sig = files.find((f) =>
    macArch ? f.endsWith(".app.tar.gz.sig") : f.endsWith("-setup.exe.sig"),
  );
  if (!sig) {
    console.warn(`↪ no .sig for ${target} — skipping`);
    continue;
  }
  const artifact = sig.slice(0, -".sig".length); // drop trailing `.sig`
  const signature = readFileSync(sig, "utf8").trim();

  let assetName;
  if (macArch) {
    // Rename to an arch-distinct, stable name and stage it for upload.
    assetName = `Tracker_${version}_${macArch}.app.tar.gz`;
    copyFileSync(artifact, join(outDir, assetName));
    copyFileSync(sig, join(outDir, `${assetName}.sig`));
  } else {
    // Windows installer already has a unique name; it's uploaded from the
    // artifacts dir as-is, so just reference that basename.
    assetName = basename(artifact);
  }

  platforms[platform] = { signature, url: releaseUrl(assetName) };
}

if (Object.keys(platforms).length === 0) {
  console.error("no updater platforms resolved — aborting");
  process.exit(1);
}

const feed = {
  version,
  notes: process.env.RELEASE_NOTES || `Tracker ${version}`,
  pub_date: new Date().toISOString(),
  platforms,
};

writeFileSync(join(outDir, "latest.json"), `${JSON.stringify(feed, null, 2)}\n`);
console.log(`Wrote ${join(outDir, "latest.json")} with platforms: ${Object.keys(platforms).join(", ")}`);

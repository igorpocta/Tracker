#!/usr/bin/env node
/**
 * Rasterize the master Tracker icon SVG → 1024×1024 PNG, plus the tray
 * template SVGs → 22×22 PNGs (used by the macOS tray).
 *
 * After this runs, `cargo tauri icon src-tauri/icons/icon-master.png` will
 * produce all .icns / .ico / per-size derivatives. We then overwrite the
 * auto-generated `tray.png` / `tray-running.png` with our monochrome versions.
 *
 * Run via `npm run icons:rasterize` or directly.
 */
import { promises as fs } from "node:fs";
import path from "node:path";
import sharp from "sharp";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const iconsDir = path.join(repoRoot, "src-tauri", "icons");

async function rasterize(svgRelPath, pngRelPath, size) {
  const svgPath = path.join(iconsDir, svgRelPath);
  const pngPath = path.join(iconsDir, pngRelPath);
  const svg = await fs.readFile(svgPath);
  await sharp(svg, { density: 1024 })
    .resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png({ compressionLevel: 9 })
    .toFile(pngPath);
  console.log(`  ${svgRelPath} → ${pngRelPath} (${size}×${size})`);
}

async function main() {
  console.log("Rasterizing Tracker icons");

  // Master colour icon — 1024×1024. Used as input to `cargo tauri icon`.
  await rasterize("icon.svg", "icon-master.png", 1024);

  // macOS tray templates — 22×22 logical, but we generate 44×44 for retina.
  // Tauri's tray API accepts a single PNG and scales; 44×44 gives crisp output
  // on @2x displays. The image must be monochrome white-on-transparent for
  // the template behaviour (set in tauri.conf.json via `iconAsTemplate`).
  await rasterize("tray.svg", "tray.png", 44);
  await rasterize("tray-running.svg", "tray-running.png", 44);

  // Pulse base — red recording dot at 100% opacity. `tray_pulse.rs` reads
  // this once at startup and produces 7 alpha-modulated frames in memory.
  await rasterize("tray-rec-base.svg", "tray-rec-base.png", 44);

  console.log("Done.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

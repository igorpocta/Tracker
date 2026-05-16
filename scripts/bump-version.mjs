#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");

const PACKAGE_JSON = resolve(repoRoot, "package.json");
const TAURI_CONF = resolve(repoRoot, "src-tauri/tauri.conf.json");
const CARGO_TOML = resolve(repoRoot, "src-tauri/Cargo.toml");

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.-]+))?$/;

function fail(message) {
  console.error(`bump-version: ${message}`);
  process.exit(1);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, data) {
  writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`);
}

function parseSemver(version) {
  const match = SEMVER_RE.exec(version);
  if (!match) fail(`Invalid semver: ${version}`);
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ?? null,
  };
}

function formatSemver({ major, minor, patch, prerelease }) {
  const base = `${major}.${minor}.${patch}`;
  return prerelease ? `${base}-${prerelease}` : base;
}

function computeNewVersion(currentRaw, input) {
  const current = parseSemver(currentRaw);
  if (input === "patch") {
    return formatSemver({ ...current, patch: current.patch + 1, prerelease: null });
  }
  if (input === "minor") {
    return formatSemver({
      ...current,
      minor: current.minor + 1,
      patch: 0,
      prerelease: null,
    });
  }
  if (input === "major") {
    return formatSemver({
      major: current.major + 1,
      minor: 0,
      patch: 0,
      prerelease: null,
    });
  }
  parseSemver(input);
  return input;
}

function bumpCargoToml(newVersion) {
  const original = readFileSync(CARGO_TOML, "utf8");
  const lines = original.split("\n");

  let inPackageSection = false;
  let bumped = false;

  const updated = lines.map((line) => {
    const sectionMatch = /^\s*\[([^\]]+)]\s*$/.exec(line);
    if (sectionMatch) {
      inPackageSection = sectionMatch[1].trim() === "package";
      return line;
    }
    if (inPackageSection && !bumped) {
      const versionMatch = /^(\s*version\s*=\s*)"[^"]+"(.*)$/.exec(line);
      if (versionMatch) {
        bumped = true;
        return `${versionMatch[1]}"${newVersion}"${versionMatch[2]}`;
      }
    }
    return line;
  });

  if (!bumped) {
    fail(`Could not find [package] version field in ${CARGO_TOML}`);
  }

  writeFileSync(CARGO_TOML, updated.join("\n"));
}

function main() {
  const input = process.argv[2];
  if (!input) {
    fail("Usage: npm run version:bump -- <patch|minor|major|X.Y.Z[-tag]>");
  }

  const pkg = readJson(PACKAGE_JSON);
  const currentVersion = pkg.version;
  if (!currentVersion) fail("package.json has no version field");

  const newVersion = computeNewVersion(currentVersion, input);

  if (newVersion === currentVersion) {
    fail(`New version (${newVersion}) is identical to current — refusing to write.`);
  }

  pkg.version = newVersion;
  writeJson(PACKAGE_JSON, pkg);

  const tauri = readJson(TAURI_CONF);
  tauri.version = newVersion;
  writeJson(TAURI_CONF, tauri);

  bumpCargoToml(newVersion);

  console.log(`Bumped: ${currentVersion} → ${newVersion}`);
  console.log("  package.json");
  console.log("  src-tauri/tauri.conf.json");
  console.log("  src-tauri/Cargo.toml");
}

main();

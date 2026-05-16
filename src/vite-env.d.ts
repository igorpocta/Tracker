/// <reference types="vite/client" />

/**
 * App version injected at build-time by `vite.config.ts`. Reads
 * `package.json#version` so the bundle always carries the same number as
 * the surrounding Rust crate / Tauri config (kept in sync manually).
 */
declare const __APP_VERSION__: string;

/**
 * Short git SHA at build time, also injected by `vite.config.ts`. Equals
 * the string `"unknown"` outside a git checkout.
 */
declare const __COMMIT_HASH__: string;

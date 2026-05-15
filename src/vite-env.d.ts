/// <reference types="vite/client" />

/**
 * App version injected at build-time by `vite.config.ts`. Reads
 * `package.json#version` so the bundle always carries the same number as
 * the surrounding Rust crate / Tauri config (kept in sync manually).
 */
declare const __APP_VERSION__: string;

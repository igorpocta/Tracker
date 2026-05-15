import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { readFileSync } from "fs";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const pkg = JSON.parse(
  readFileSync(resolve(__dirname, "package.json"), "utf8"),
) as { version: string };

// https://vite.dev/config/
export default defineConfig(async ({ mode }) => {
  // Phase 19: pick up `TRACKER_SENTRY_DSN_FRONTEND` from the build shell
  // (or a local .env) and surface it under the `VITE_` prefix the SDK
  // reads at runtime. We do NOT define a fallback — when the var is
  // missing, the bundle ships with `undefined` and Sentry stays off.
  const env = loadEnv(mode, process.cwd(), "");
  const sentryDsn = env.TRACKER_SENTRY_DSN_FRONTEND ?? "";

  return {
    plugins: [react(), tailwindcss()],

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
    define: {
      // Phase 19: embed the package version so Sentry events get a
      // matching `release` tag and our frontend can show the same
      // string we ship.
      __APP_VERSION__: JSON.stringify(pkg.version),
      // Phase 19: surface the optional build-time DSN under the
      // canonical `import.meta.env.VITE_*` key. Empty string ≡ no DSN.
      "import.meta.env.VITE_TRACKER_SENTRY_DSN_FRONTEND": JSON.stringify(sentryDsn),
    },
    build: {
      // Production: žádné sourcemaps, ať se z bundlu nedá triviálně
      // zrekonstruovat zdroj. `scripts/build-release.sh` (Sentry release)
      // si je zapne přes `VITE_RELEASE` — generují se jako `hidden`
      // (vznikají vedle bundlu, ale script je po uploadu do Sentry maže
      // z dist/, tj. nešíří se v aplikační instalaci).
      sourcemap: env.VITE_RELEASE ? "hidden" : false,
      rollupOptions: {
        input: {
          main: resolve(__dirname, "index.html"),
          popover: resolve(__dirname, "popover.html"),
        },
      },
    },
    // Production build: esbuild zahodí všechna `console.*` volání a
    // `debugger` statementy. V dev módu (`npm run dev`) zůstávají, ať
    // máme logy při ladění.
    esbuild:
      mode === "production" ? { drop: ["console", "debugger"] } : undefined,
  };
});

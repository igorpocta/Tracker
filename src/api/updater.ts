/**
 * Auto-update wrappers around `@tauri-apps/plugin-updater` +
 * `@tauri-apps/plugin-process`.
 *
 * The update feed is a signed `latest.json` published on GitHub Releases
 * (see `tauri.conf.json → plugins.updater`). Signature verification is done
 * natively by the plugin against the configured public key — a tampered
 * artifact is rejected before install.
 *
 * These thin wrappers exist so the store (and its tests) don't import the
 * Tauri plugins directly, which keeps them mockable in jsdom.
 */
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type { Update };

/**
 * Check the update endpoint for a newer signed build. Returns the `Update`
 * handle (carrying `.version` / `.body`) when one is available, otherwise
 * `null`. Network / endpoint errors reject.
 */
export async function checkForUpdate(): Promise<Update | null> {
  return check();
}

/**
 * Download + install `update`, reporting cumulative byte progress. Does NOT
 * relaunch — the caller decides when (never while a timer is running).
 */
export async function downloadAndInstall(
  update: Update,
  onProgress?: (downloaded: number, total: number | null) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        onProgress?.(0, total);
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        onProgress?.(downloaded, total);
        break;
      case "Finished":
        onProgress?.(total ?? downloaded, total);
        break;
    }
  });
}

/** Restart the app to finish applying a downloaded update. */
export async function relaunchApp(): Promise<void> {
  await relaunch();
}

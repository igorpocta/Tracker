/**
 * Auto-update state machine.
 *
 * Product rules (time-tracker specific):
 *  - never restart silently — the user always presses an explicit "Restart &
 *    finish" (the banner also refuses to auto-restart while a timer is running);
 *  - a background check fires a few seconds after launch and then at most once
 *    per day (throttled via a persisted `lastCheckedAt`);
 *  - Settings → About offers a manual "check now" button.
 *
 * The banner UI (timer-aware messaging) lives in `UpdateBanner`; this store
 * only tracks status and drives download/relaunch.
 */
import { create } from "zustand";

import {
  checkForUpdate,
  downloadAndInstall,
  relaunchApp,
  type Update,
} from "../api/updater";

export type UpdaterStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

const LAST_CHECK_KEY = "tracker.updater.lastCheckedAt";
/** Background auto-check throttle: at most once per 24 h. */
export const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

function loadLastChecked(): number {
  try {
    const raw = localStorage.getItem(LAST_CHECK_KEY);
    return raw ? Number(raw) || 0 : 0;
  } catch {
    return 0;
  }
}

function saveLastChecked(ts: number): void {
  try {
    localStorage.setItem(LAST_CHECK_KEY, String(ts));
  } catch {
    // localStorage unavailable (private mode / tests) — throttle just resets.
  }
}

interface UpdaterState {
  status: UpdaterStatus;
  /** Version offered by the feed (only meaningful in available/downloading/ready). */
  version: string | null;
  /** Release notes / changelog body from the feed, if any. */
  notes: string | null;
  /** Download progress in bytes; `total` is null until the feed reports it. */
  downloaded: number;
  total: number | null;
  error: string | null;
  lastCheckedAt: number;
  /** The live Update handle — non-serialisable, kept only in memory. */
  _update: Update | null;

  /**
   * Check the feed. `silent` (the startup / daily path) swallows the
   * "no update / error" states back to idle so nothing pops up unprompted;
   * the manual button passes `silent: false` to surface the result.
   */
  check: (opts?: { silent?: boolean }) => Promise<void>;
  /** Check only if the daily throttle has elapsed (startup path). */
  maybeCheckOnStartup: (now?: number) => Promise<void>;
  /** Download + install the pending update (does not relaunch). */
  download: () => Promise<void>;
  /** Restart to finish applying a downloaded update. */
  relaunch: () => Promise<void>;
  /** Dismiss the banner back to idle. */
  dismiss: () => void;
}

export const useUpdaterStore = create<UpdaterState>((set, get) => ({
  status: "idle",
  version: null,
  notes: null,
  downloaded: 0,
  total: null,
  error: null,
  lastCheckedAt: loadLastChecked(),
  _update: null,

  check: async (opts) => {
    const silent = opts?.silent ?? false;
    // Don't clobber an in-flight download / ready state with a re-check.
    if (get().status === "downloading") return;
    set({ status: "checking", error: null });
    const now = Date.now();
    saveLastChecked(now);
    set({ lastCheckedAt: now });
    try {
      const update = await checkForUpdate();
      if (update) {
        set({
          status: "available",
          version: update.version,
          notes: update.body ?? null,
          _update: update,
        });
      } else {
        // Up to date. Silent path leaves no trace; manual path also returns to
        // idle (the button shows its own "you're up to date" feedback).
        set({ status: "idle", version: null, notes: null, _update: null });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // A silent background failure must never nag the user.
      set(silent ? { status: "idle" } : { status: "error", error: msg });
    }
  },

  maybeCheckOnStartup: async (now = Date.now()) => {
    if (now - get().lastCheckedAt < CHECK_INTERVAL_MS) return;
    await get().check({ silent: true });
  },

  download: async () => {
    const update = get()._update;
    if (!update) return;
    set({ status: "downloading", downloaded: 0, total: null, error: null });
    try {
      await downloadAndInstall(update, (downloaded, total) =>
        set({ downloaded, total }),
      );
      set({ status: "ready" });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ status: "error", error: msg });
    }
  },

  relaunch: async () => {
    await relaunchApp();
  },

  dismiss: () => set({ status: "idle", error: null }),
}));

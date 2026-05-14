/**
 * `useActivityTracker` — Phase 18A Item 32.
 *
 * Mounted once at the application shell level. Listens for `mousemove`,
 * `keydown`, and `pointerdown` events on `window`. To avoid hammering the
 * backend, events are debounced — at most one `record_user_activity` IPC call
 * per `intervalMs` (default 30s).
 *
 * The backend aggregates these into `active_seconds` / `inactive_seconds`
 * rows in `daily_activity`; the Goals view surfaces the ratio.
 *
 * This is **not** billing input. We intentionally do NOT block worklogs based
 * on inactivity — it's informational only.
 */
import { useEffect, useRef } from "react";

import { recordUserActivity } from "../api/commands";

export interface UseActivityTrackerOptions {
  /** Minimum time between backend calls. Default 30s. */
  intervalMs?: number;
  /** Disable the listener entirely (e.g. for tests). Default false. */
  disabled?: boolean;
}

export function useActivityTracker(opts: UseActivityTrackerOptions = {}): void {
  const { intervalMs = 30_000, disabled = false } = opts;
  const lastSentRef = useRef<number>(0);

  useEffect(() => {
    if (disabled) return;

    const send = () => {
      const now = Date.now();
      if (now - lastSentRef.current < intervalMs) return;
      lastSentRef.current = now;
      recordUserActivity(now).catch(() => {
        // Best-effort: activity tracking is purely informational.
      });
    };

    // Listen on the capture phase so we catch events even when handlers below
    // call `stopPropagation`.
    window.addEventListener("mousemove", send, { capture: true, passive: true });
    window.addEventListener("keydown", send, { capture: true, passive: true });
    window.addEventListener("pointerdown", send, { capture: true, passive: true });

    return () => {
      window.removeEventListener("mousemove", send, { capture: true } as EventListenerOptions);
      window.removeEventListener("keydown", send, { capture: true } as EventListenerOptions);
      window.removeEventListener("pointerdown", send, { capture: true } as EventListenerOptions);
    };
  }, [intervalMs, disabled]);
}

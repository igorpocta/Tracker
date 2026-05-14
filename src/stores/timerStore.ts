/**
 * Zustand store mirroring the backend's active timer state.
 *
 * The Rust side is the source of truth (it persists the row in SQLite), so
 * this store is essentially a frontend cache that the React tree subscribes
 * to. The flow is:
 *
 *   1. On mount the `Home` route calls `hydrate()` which `invoke`s
 *      `get_timer_state` and seeds the store.
 *   2. Mutating actions (`start`, `stop`, `updateStart`) call the Tauri
 *      command then patch the local snapshot.
 *   3. The `timer-started` and `worklog-saved` events emitted by the backend
 *      also feed back into the store via dedicated setters (subscribers wire
 *      these up themselves).
 */
import { create } from "zustand";

import {
  getTimerState,
  startTimer as invokeStart,
  stopTimer as invokeStop,
  updateTimerStart as invokeUpdateStart,
} from "../api/commands";
import type { ActiveTimerState, WorklogRow } from "../api/types";

export interface TimerStoreState {
  /** Snapshot from the backend; null when no timer is running. */
  active: ActiveTimerState | null;
  /** True while a command is in-flight (start / stop / update). */
  busy: boolean;
  /** Optional surfaced error from the last operation. */
  error: string | null;
}

export interface TimerStoreActions {
  /** Pull current state from the backend; safe to call repeatedly. */
  hydrate: () => Promise<void>;
  /** Start (or restart) the timer for the given issue. */
  start: (issueKey: string) => Promise<void>;
  /** Stop the active timer with an optional comment. */
  stop: (comment?: string) => Promise<WorklogRow | null>;
  /** Adjust the start time of the running timer (ms since epoch). */
  updateStart: (startedAtMs: number) => Promise<void>;
  /** Setter used by event subscribers (`timer-started` / external sources). */
  setActive: (next: ActiveTimerState | null) => void;
  /** Imperatively clear local state (e.g. on `worklog-saved`). */
  clear: () => void;
}

export type TimerStore = TimerStoreState & TimerStoreActions;

export const useTimerStore = create<TimerStore>((set, get) => ({
  active: null,
  busy: false,
  error: null,

  hydrate: async () => {
    try {
      const next = await getTimerState();
      set({ active: next ?? null, error: null });
    } catch (e) {
      set({ error: errMessage(e) });
    }
  },

  start: async (issueKey) => {
    set({ busy: true, error: null });
    try {
      const next = await invokeStart(issueKey);
      set({ active: next, busy: false });
    } catch (e) {
      set({ busy: false, error: errMessage(e) });
      throw e;
    }
  },

  stop: async (comment) => {
    set({ busy: true, error: null });
    try {
      const row = await invokeStop(comment);
      set({ active: null, busy: false });
      return row;
    } catch (e) {
      set({ busy: false, error: errMessage(e) });
      throw e;
    }
  },

  updateStart: async (startedAtMs) => {
    if (!get().active) return;
    set({ busy: true, error: null });
    try {
      const next = await invokeUpdateStart(startedAtMs);
      set({ active: next, busy: false });
    } catch (e) {
      set({ busy: false, error: errMessage(e) });
      throw e;
    }
  },

  setActive: (next) => set({ active: next }),
  clear: () => set({ active: null }),
}));

/** Compute the elapsed seconds for the current timer at a given `now` (ms). */
export function elapsedSeconds(
  active: ActiveTimerState | null,
  nowMs: number,
): number {
  if (!active) return 0;
  return Math.max(0, Math.floor((nowMs - active.started_at) / 1000));
}

function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "unknown error";
}

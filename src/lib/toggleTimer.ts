import type { ActiveTimerState } from "../api/types";

/**
 * Shared toggle logic for the global shortcut (and any other "one button does
 * both" surface). Reads the *backend* timer state via `getActive` rather than a
 * possibly-stale local snapshot, then stops if running or starts an unassigned
 * timer if idle. No-ops while another timer command is in flight so a double
 * key-press can't race start against stop.
 */
export interface ToggleTimerDeps {
  /** True while a start/stop/update command is already running. */
  isBusy: () => boolean;
  /** Authoritative current timer state from the backend. */
  getActive: () => Promise<ActiveTimerState | null>;
  /** Start an unassigned timer. */
  start: () => Promise<void>;
  /** Stop the running timer (records the worklog). */
  stop: () => Promise<unknown>;
}

export async function toggleTimer(deps: ToggleTimerDeps): Promise<void> {
  if (deps.isBusy()) return;
  const active = await deps.getActive();
  if (active) {
    await deps.stop();
  } else {
    await deps.start();
  }
}

/**
 * Helpers for the "idle detected" flow (Toggl-style discard / discard+continue).
 */

/**
 * New timer `started_at` (ms) after discarding an idle gap by shifting the
 * start forward by `idleMs`. Clamped to `[startedAtMs, nowMs]` so it can never
 * land before the original start or past `now` — the latter would make the
 * stopped worklog zero/negative duration, which the backend rejects, silently
 * losing the work done before the user went idle.
 */
export function clampDiscardStartMs(
  startedAtMs: number,
  idleMs: number,
  nowMs: number,
): number {
  const shifted = startedAtMs + Math.max(0, idleMs);
  return Math.min(Math.max(shifted, startedAtMs), nowMs);
}

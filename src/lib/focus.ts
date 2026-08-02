/**
 * Focus-mode formatting helpers.
 *
 * Lives in `lib` rather than next to the Settings panel because the sidebar
 * and the popover show the same countdown, and neither should have to pull in
 * a settings screen to render a clock.
 */

/**
 * Countdown for a running session: `90` → `1:30`, `3600` → `1:00:00`.
 *
 * Distinct from `formatDuration` in `lib/format`, which renders logged time as
 * `1h 30m`. A countdown ticking every second needs seconds visible and a
 * fixed-width shape so the label doesn't jitter.
 */
export function formatRemaining(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

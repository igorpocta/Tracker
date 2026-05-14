/**
 * Big live-ticking timer display. Reads `elapsed_seconds` from the timer
 * store and recomputes against `useNow()` every second so the display
 * never drifts more than ~16ms from the wall clock.
 */
import { clsx } from "clsx";

import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";

export interface TimerProps {
  /** Tailwind size class for the display, e.g. `text-5xl`. */
  className?: string;
}

export function Timer({ className }: TimerProps) {
  const active = useTimerStore((s) => s.active);
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);
  const running = active !== null;

  return (
    <div
      className={clsx(
        "font-mono tabular-nums leading-none",
        running ? "text-white" : "text-neutral-600",
        className,
      )}
      aria-live="polite"
      aria-label={running ? `Elapsed time ${formatDuration(elapsed)}` : "Timer not running"}
    >
      {running ? formatDuration(elapsed) : "--:--:--"}
    </div>
  );
}

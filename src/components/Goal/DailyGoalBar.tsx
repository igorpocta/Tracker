/**
 * Daily progress bar: how much of today's goal the user has logged.
 *
 * Bar fills proportionally — the accent color until the goal is reached, then
 * the success color. The label keeps growing ("9h 15m / 8h") past goal.
 */
import { clsx } from "clsx";

import { formatDurationShort } from "../../lib/format";

export interface DailyGoalBarProps {
  /** Seconds logged today (incl. running timer elapsed). */
  loggedSeconds: number;
  /** Seconds in the daily goal. */
  goalSeconds: number;
  className?: string;
}

export function DailyGoalBar({
  loggedSeconds,
  goalSeconds,
  className,
}: DailyGoalBarProps) {
  const safeGoal = Math.max(1, goalSeconds);
  const ratio = Math.max(0, loggedSeconds) / safeGoal;
  const clamped = Math.min(1, ratio);
  const reached = ratio >= 1;

  return (
    <div className={clsx("flex flex-col gap-1.5", className)}>
      <div className="flex items-baseline justify-between text-xs">
        <span className="text-[var(--text-tertiary)]">Today</span>
        <span className="font-mono tabular-nums text-[var(--text-primary)]">
          {formatDurationShort(loggedSeconds)}{" "}
          <span className="text-[var(--text-tertiary)]">
            / {formatDurationShort(goalSeconds)}
          </span>
        </span>
      </div>
      <div
        className="h-1.5 rounded-full bg-[var(--bg-active)] overflow-hidden"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={goalSeconds}
        aria-valuenow={Math.min(loggedSeconds, goalSeconds)}
        aria-label="Daily goal progress"
      >
        <div
          className={clsx(
            "h-full transition-all duration-200 ease-out",
            reached ? "bg-[var(--success)]" : "bg-[var(--accent)]",
          )}
          style={{ width: `${(clamped * 100).toFixed(1)}%` }}
        />
      </div>
    </div>
  );
}

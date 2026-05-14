/**
 * Daily progress bar: how much of today's goal the user has logged.
 *
 * The "logged" value is computed by the parent (typically by summing
 * today's worklogs + current running timer's elapsed). The bar fills
 * proportionally; once we exceed the goal the bar caps at 100% but the
 * label keeps growing ("9h 15m / 8h").
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
    <div className={clsx("flex flex-col gap-1", className)}>
      <div className="flex items-baseline justify-between text-xs">
        <span className="text-neutral-400">Today</span>
        <span className="font-mono tabular-nums text-neutral-200">
          {formatDurationShort(loggedSeconds)}{" "}
          <span className="text-neutral-500">
            / {formatDurationShort(goalSeconds)}
          </span>
        </span>
      </div>
      <div
        className="h-2 rounded-full bg-neutral-800 overflow-hidden"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={goalSeconds}
        aria-valuenow={Math.min(loggedSeconds, goalSeconds)}
        aria-label="Daily goal progress"
      >
        <div
          className={clsx(
            "h-full transition-all",
            reached ? "bg-emerald-500" : "bg-sky-500",
          )}
          style={{ width: `${(clamped * 100).toFixed(1)}%` }}
        />
      </div>
    </div>
  );
}

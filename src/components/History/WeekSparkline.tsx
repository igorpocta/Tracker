/**
 * Tiny inline week sparkline — a 7-bar chart of Mon..Sun daily totals.
 *
 * Used at the top of the History route to give the user a sense of where
 * the selected day falls within the current week.
 */
import { clsx } from "clsx";
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";
import { isSameDay, weekDays } from "../../lib/dates";

export interface WeekSparklineProps {
  rows: WorklogRow[];
  selected: Date;
  onSelect?: (date: Date) => void;
}

export function WeekSparkline({ rows, selected, onSelect }: WeekSparklineProps) {
  const days = useMemo(() => weekDays(selected), [selected]);

  // Sum durations per day.
  const totals = useMemo(() => {
    const arr = days.map(() => 0);
    for (const r of rows) {
      const d = new Date(r.started_at * 1000);
      const idx = days.findIndex((day) => isSameDay(day, d));
      if (idx >= 0) arr[idx] += r.duration_s;
    }
    return arr;
  }, [rows, days]);

  const maxTotal = Math.max(1, ...totals);
  const weekTotal = totals.reduce((a, b) => a + b, 0);

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <h3 className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          This week
        </h3>
        <span className="text-xs text-[var(--text-primary)] font-mono tabular-nums">
          {formatDurationShort(weekTotal)}
        </span>
      </div>
      <div className="flex items-end gap-1.5 h-16">
        {days.map((d, i) => {
          const ratio = totals[i] / maxTotal;
          const isSel = isSameDay(d, selected);
          return (
            <button
              key={i}
              type="button"
              onClick={() => onSelect?.(d)}
              aria-label={`${d.toDateString()} — ${formatDurationShort(totals[i])}`}
              className="flex-1 flex flex-col items-center gap-1 group"
            >
              <div className="w-full h-12 flex items-end">
                <div
                  className={clsx(
                    "w-full rounded-t transition-colors duration-150",
                    totals[i] === 0
                      ? "bg-[var(--bg-active)]"
                      : isSel
                        ? "bg-[var(--accent)]"
                        : "bg-[var(--accent)]/55 group-hover:bg-[var(--accent)]/80",
                  )}
                  style={{
                    height: `${Math.max(3, Math.floor(ratio * 100))}%`,
                  }}
                />
              </div>
              <span
                className={clsx(
                  "text-[10px] uppercase tracking-tight",
                  isSel ? "text-[var(--accent)]" : "text-[var(--text-tertiary)]",
                )}
              >
                {weekdayShort(d)}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function weekdayShort(d: Date): string {
  return new Intl.DateTimeFormat(undefined, { weekday: "short" })
    .format(d)
    .replace(/\.$/, "");
}

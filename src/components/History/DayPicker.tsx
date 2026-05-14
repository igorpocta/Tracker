/**
 * Vertical day-picker rail used on the History route.
 *
 * Shows the last `count` days as clickable rows, each with a small total-time
 * bar to give the user a quick visual sense of how busy the day was. The
 * top row is always pinned as "Today".
 */
import { clsx } from "clsx";
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";
import {
  dayEndUnixS,
  dayStartUnixS,
  formatShortDayLabel,
  isSameDay,
  lastNDays,
} from "../../lib/dates";

export interface DayPickerProps {
  /** All worklog rows in scope (last N days). */
  rows: WorklogRow[];
  /** Currently selected day. */
  selected: Date;
  /** Number of recent days to render. */
  count?: number;
  onSelect: (date: Date) => void;
}

export function DayPicker({
  rows,
  selected,
  count = 30,
  onSelect,
}: DayPickerProps) {
  const today = new Date();
  const days = useMemo(() => lastNDays(today, count), [today, count]);

  // Pre-compute totals per day for the bars.
  const totals = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of rows) {
      const d = new Date(r.started_at * 1000);
      const key = formatKey(d);
      map.set(key, (map.get(key) ?? 0) + r.duration_s);
    }
    return map;
  }, [rows]);

  const max = Math.max(1, ...Array.from(totals.values()));

  return (
    <ul className="flex flex-col gap-0.5" role="list" aria-label="Recent days">
      {days.map((d, idx) => {
        const key = formatKey(d);
        const total = totals.get(key) ?? 0;
        const isSelected = isSameDay(d, selected);
        const isToday = isSameDay(d, today);
        const ratio = total / max;
        return (
          <li key={key}>
            <button
              type="button"
              onClick={() => onSelect(d)}
              aria-current={isSelected ? "date" : undefined}
              data-from={dayStartUnixS(d)}
              data-to={dayEndUnixS(d)}
              className={clsx(
                "w-full text-left rounded-[var(--radius-sm)] px-2.5 py-1.5 transition-colors duration-150 flex items-center gap-2",
                isSelected
                  ? "bg-[var(--accent-soft)] text-[var(--text-primary)]"
                  : "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]",
              )}
            >
              <div className="flex-1 min-w-0">
                <div className="text-xs">
                  {isToday ? (
                    <span className="text-[var(--accent)] font-medium">Today</span>
                  ) : idx === 1 ? (
                    <span className="text-[var(--text-primary)]">Yesterday</span>
                  ) : (
                    <span className="text-[var(--text-primary)]">{formatShortDayLabel(d)}</span>
                  )}
                </div>
                <div className="text-[10px] text-[var(--text-tertiary)] font-mono tabular-nums">
                  {total > 0 ? formatDurationShort(total) : "—"}
                </div>
              </div>
              <div className="w-12 h-1 rounded-full bg-[var(--bg-active)] overflow-hidden shrink-0">
                <div
                  className={clsx(
                    "h-full transition-all duration-200",
                    isSelected ? "bg-[var(--accent)]" : "bg-[var(--text-disabled)]",
                  )}
                  style={{ width: `${(ratio * 100).toFixed(1)}%` }}
                />
              </div>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function formatKey(date: Date): string {
  return `${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`;
}

/**
 * Daily-hours bar chart used on the Reports route.
 *
 * Phase 18B improvements:
 *   - Item 1: working-day shading. Working day columns get a faint
 *     `--accent-soft` background tint; weekends/holidays sit on `--bg-app`.
 *     The chart pulls the user's working-week mask + non-working-day list
 *     once per range and derives per-column state locally — no per-day RPC.
 *   - Item 21: tooltip on hover, fixed scale calculation. The y-axis now
 *     anchors to `max(observed_max, dailyGoalHours)` so the bars never look
 *     1mm tall just because the day-goal is high. The bar minimum height is
 *     also a more visible 3px (was 2px).
 */
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import type { WorklogRow } from "../../api/types";
import { isWorkingDayLocal, useCalendarMask } from "../../hooks/useCalendarMask";
import { addDays, startOfDay } from "../../lib/dates";
import { formatDateCs, formatDateCsShort, formatDurationShort } from "../../lib/format";

export interface DailyBarChartProps {
  rows: WorklogRow[];
  from: Date;
  to: Date;
  /** Optional daily-goal anchor for the y-axis, in hours. */
  dailyGoalHours?: number;
}

export function DailyBarChart({ rows, from, to, dailyGoalHours }: DailyBarChartProps) {
  const days = useMemo(() => buildDayList(from, to), [from, to]);
  const totals = useMemo(() => totalsByDay(rows), [rows]);
  const counts = useMemo(() => countsByDay(rows), [rows]);

  // Phase 18B — Item 1: per-day working-day state.
  const { mask, nonWorking } = useCalendarMask(from, to);

  const observedMaxHours = useMemo(() => {
    let m = 0;
    for (const d of days) {
      const v = (totals.get(formatKey(d)) ?? 0) / 3600;
      if (v > m) m = v;
    }
    return m;
  }, [days, totals]);

  // Anchor the y-axis to MAX(observed, dailyGoalHours, 3) rounded up to a
  // multiple of 3 for readability.
  const max = useMemo(() => {
    const base = Math.max(observedMaxHours, dailyGoalHours ?? 0, 3);
    return Math.max(3, Math.ceil(base / 3) * 3);
  }, [observedMaxHours, dailyGoalHours]);

  const [hover, setHover] = useState<number | null>(null);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
        Hodiny za den
      </h3>
      <div className="flex gap-4 h-[260px]">
        <div className="flex flex-col justify-between text-[10px] text-[var(--text-tertiary)] tabular-nums py-1">
          {[max, Math.round((max * 2) / 3), Math.round(max / 3), 0].map((v) => (
            <div key={`y-${v}`}>{v}</div>
          ))}
        </div>
        <div className="flex-1 relative">
          {/* Grid lines */}
          <div className="absolute inset-0 flex flex-col justify-between py-1 pointer-events-none">
            {[0, 1, 2, 3].map((i) => (
              <div
                key={`grid-${i}`}
                className="border-t border-[var(--border-subtle)]"
              />
            ))}
          </div>
          {/* Working-day shading bands (sit behind the bars) */}
          <div className="absolute inset-0 flex gap-[2px] pt-1 pb-5 pointer-events-none">
            {days.map((d) => {
              const working = isWorkingDayLocal(d, mask, nonWorking);
              return (
                <div
                  key={`band-${d.toISOString()}`}
                  className="flex-1 rounded-[2px]"
                  style={{
                    background: working
                      ? "var(--accent-soft)"
                      : "transparent",
                    opacity: working ? 0.35 : 0,
                  }}
                />
              );
            })}
          </div>
          {/* Bars */}
          <div className="absolute inset-0 flex items-end gap-[2px] pt-1 pb-5">
            {days.map((d, idx) => {
              const seconds = totals.get(formatKey(d)) ?? 0;
              const hours = seconds / 3600;
              const heightPct = max > 0 ? (hours / max) * 100 : 0;
              const isHovered = hover === idx;
              return (
                <div
                  key={d.toISOString()}
                  className="flex-1 relative cursor-pointer"
                  onMouseEnter={() => setHover(idx)}
                  onMouseLeave={() => setHover(null)}
                >
                  <div
                    className="absolute left-0 right-0 bottom-0 rounded-t-[2px] transition-colors duration-150"
                    style={{
                      height: `${heightPct}%`,
                      minHeight: heightPct > 0 ? "3px" : 0,
                      background:
                        heightPct > 0
                          ? isHovered
                            ? "var(--accent-hover)"
                            : "var(--accent)"
                          : "transparent",
                    }}
                  />
                  {isHovered && seconds > 0 && (
                    <DailyTooltip
                      date={d}
                      seconds={seconds}
                      count={counts.get(formatKey(d)) ?? 0}
                    />
                  )}
                </div>
              );
            })}
          </div>
          {/* X-axis labels */}
          <div className="absolute left-0 right-0 bottom-0 flex gap-[2px]">
            {days.map((d, idx) => (
              <div
                key={`label-${d.toISOString()}`}
                className="flex-1 text-[9px] text-[var(--text-tertiary)] text-center tabular-nums"
              >
                {idx % 2 === 0 ? formatDateCsShort(d) : ""}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function DailyTooltip({
  date,
  seconds,
  count,
}: {
  date: Date;
  seconds: number;
  count: number;
}) {
  return (
    <div
      role="tooltip"
      className="absolute left-1/2 -translate-x-1/2 bottom-[calc(100%+4px)] z-20
                 px-2 py-1 rounded-[var(--radius-sm)] text-[11px] whitespace-nowrap
                 pointer-events-none"
      style={{
        background: "var(--bg-elevated)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div className="font-medium">{formatDateCs(date)}</div>
      <div className="text-[var(--text-tertiary)]">
        {formatDurationShort(seconds)}
        {count > 1 ? ` · ${count} záznamů` : count === 1 ? ` · 1 záznam` : ""}
      </div>
    </div>
  );
}

function buildDayList(from: Date, to: Date): Date[] {
  const out: Date[] = [];
  const d = startOfDay(from);
  const end = startOfDay(to);
  while (d <= end) {
    out.push(new Date(d));
    d.setDate(d.getDate() + 1);
  }
  return out;
}

function totalsByDay(rows: WorklogRow[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const r of rows) {
    const d = new Date(r.started_at * 1000);
    const k = formatKey(d);
    map.set(k, (map.get(k) ?? 0) + r.duration_s);
  }
  return map;
}

function countsByDay(rows: WorklogRow[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const r of rows) {
    const d = new Date(r.started_at * 1000);
    const k = formatKey(d);
    map.set(k, (map.get(k) ?? 0) + 1);
  }
  return map;
}

export function formatKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

// Re-export the date arithmetic for the chart-only consumers in tests.
export { addDays, useQuery };

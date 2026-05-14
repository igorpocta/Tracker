/**
 * Inline SVG bar chart: one bar per day in the supplied range.
 *
 * Renders fully responsively using a `viewBox` on the `<svg>`; bar width is
 * computed so the chart always uses the full horizontal space available.
 */
import { useMemo } from "react";

import {
  addDays,
  daysBetween,
  isSameDay,
  startOfDay,
} from "../../lib/dates";
import { formatDurationShort } from "../../lib/format";
import type { WorklogRow } from "../../api/types";

export interface DailyBarChartProps {
  from: Date;
  to: Date;
  rows: WorklogRow[];
  /** Optional click handler — useful so users can drill into a day. */
  onSelectDay?: (date: Date) => void;
}

export function DailyBarChart({ from, to, rows, onSelectDay }: DailyBarChartProps) {
  const data = useMemo(() => {
    const n = Math.max(1, daysBetween(startOfDay(from), startOfDay(to)) + 1);
    const days = Array.from({ length: n }, (_, i) => addDays(startOfDay(from), i));
    const totals = days.map(() => 0);
    for (const r of rows) {
      const d = new Date(r.started_at * 1000);
      const idx = days.findIndex((day) => isSameDay(day, d));
      if (idx >= 0) totals[idx] += r.duration_s;
    }
    return { days, totals };
  }, [from, to, rows]);

  const max = Math.max(1, ...data.totals);
  // Dynamic chart sizing.
  const width = Math.max(360, data.days.length * 28);
  const height = 200;
  const marginTop = 12;
  const marginBottom = 22;
  const marginLeft = 36;
  const marginRight = 8;
  const innerW = width - marginLeft - marginRight;
  const innerH = height - marginTop - marginBottom;
  const barGap = 4;
  const barW = Math.max(2, innerW / data.days.length - barGap);

  // Y-axis ticks: 0, 25%, 50%, 75%, 100% of max.
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((p) => p * max);

  return (
    <div className="w-full overflow-x-auto" data-testid="daily-bar-chart">
      <svg
        role="img"
        aria-label="Daily worklog totals"
        viewBox={`0 0 ${width} ${height}`}
        className="w-full h-[200px] min-w-[360px]"
      >
        {/* Gridlines + Y labels. */}
        {ticks.map((t, i) => {
          const y = marginTop + innerH - (t / max) * innerH;
          return (
            <g key={i}>
              <line
                x1={marginLeft}
                x2={width - marginRight}
                y1={y}
                y2={y}
                stroke="rgba(255,255,255,0.08)"
                strokeWidth={1}
              />
              <text
                x={marginLeft - 4}
                y={y + 3}
                fill="rgba(255,255,255,0.4)"
                fontSize={9}
                textAnchor="end"
                fontFamily="monospace"
              >
                {`${Math.round((t / 3600) * 10) / 10}h`}
              </text>
            </g>
          );
        })}
        {/* Bars. */}
        {data.days.map((d, i) => {
          const total = data.totals[i];
          const h = (total / max) * innerH;
          const x = marginLeft + i * (barW + barGap);
          const y = marginTop + innerH - h;
          return (
            <g key={i}>
              {onSelectDay ? (
                <rect
                  x={x}
                  y={marginTop}
                  width={barW}
                  height={innerH}
                  fill="transparent"
                  className="cursor-pointer"
                  onClick={() => onSelectDay(d)}
                  aria-label={`${d.toDateString()} — ${formatDurationShort(total)}`}
                >
                  <title>{`${d.toDateString()} — ${formatDurationShort(total)}`}</title>
                </rect>
              ) : null}
              <rect
                x={x}
                y={y}
                width={barW}
                height={Math.max(1, h)}
                rx={2}
                fill={total === 0 ? "rgba(255,255,255,0.06)" : "rgb(56 189 248)"}
                opacity={total === 0 ? 0.8 : 0.85}
                className="transition-opacity"
              >
                <title>{`${d.toDateString()} — ${formatDurationShort(total)}`}</title>
              </rect>
              {/* Optional X label — every Nth bar to avoid overlap. */}
              {labelFor(i, data.days.length) && (
                <text
                  x={x + barW / 2}
                  y={height - 6}
                  fill="rgba(255,255,255,0.4)"
                  fontSize={9}
                  textAnchor="middle"
                  fontFamily="monospace"
                >
                  {`${d.getDate()}/${d.getMonth() + 1}`}
                </text>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function labelFor(i: number, n: number): boolean {
  if (n <= 14) return true;
  if (n <= 31) return i % 3 === 0;
  return i % 5 === 0;
}

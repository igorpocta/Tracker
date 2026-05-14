/**
 * Day overview timeline — Phase 18B Item 31.
 *
 * Renders today's worklogs as labeled bars on a 06:00–22:00 axis. Each bar
 * shows the issue key (truncated as needed), the user can hover for the full
 * summary + duration, and clicking a bar fires `onSelect(worklog)` so the
 * outer view can scroll/focus the matching row in the worklog list.
 *
 *   06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22
 *           [DEV-792==========][DEV-304========][DEV-926=]
 */
import { clsx } from "clsx";
import { useState } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";

/** First hour shown on the axis. */
const START_HOUR = 6;
/** Last hour shown (exclusive). */
const END_HOUR = 22;

export interface DayTimelineProps {
  rows: WorklogRow[];
  /** The day represented by the timeline (used for clamping). */
  day: Date;
  /** Optional callback when the user clicks a bar. */
  onSelect?: (row: WorklogRow) => void;
}

interface Segment {
  row: WorklogRow;
  /** Bar start in fractional hours from START_HOUR. */
  leftFrac: number;
  /** Bar width in fractional hours. */
  widthFrac: number;
}

export function DayTimeline({ rows, day, onSelect }: DayTimelineProps) {
  const segments = buildSegments(rows, day);
  const totalHours = END_HOUR - START_HOUR;
  const [hover, setHover] = useState<number | null>(null);

  return (
    <div
      className="rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] p-3"
      aria-label="Časová osa dne"
    >
      <h3 className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-3">
        Časová osa dne
      </h3>

      {/* Hour labels */}
      <div
        className="grid"
        style={{
          gridTemplateColumns: `repeat(${totalHours}, minmax(0, 1fr))`,
        }}
      >
        {Array.from({ length: totalHours }, (_, i) => (
          <div
            key={`h-${i}`}
            className="text-[10px] font-mono text-[var(--text-tertiary)] text-left tabular-nums"
          >
            {START_HOUR + i}
          </div>
        ))}
      </div>

      {/* Bar track */}
      <div className="relative h-7 mt-1 rounded-[var(--radius-sm)] bg-[var(--bg-app)]">
        {/* Hour-line grid (subtle) */}
        <div
          aria-hidden
          className="absolute inset-0 grid pointer-events-none"
          style={{
            gridTemplateColumns: `repeat(${totalHours}, minmax(0, 1fr))`,
          }}
        >
          {Array.from({ length: totalHours }, (_, i) => (
            <div
              key={`grid-${i}`}
              className={clsx(
                i > 0 && "border-l border-[var(--border-subtle)]",
              )}
            />
          ))}
        </div>

        {segments.map((seg, idx) => {
          const leftPct = (seg.leftFrac / totalHours) * 100;
          const widthPct = (seg.widthFrac / totalHours) * 100;
          const isHovered = hover === idx;
          return (
            <button
              key={`${seg.row.id ?? seg.row.jira_worklog_id ?? idx}-${seg.leftFrac}`}
              type="button"
              onMouseEnter={() => setHover(idx)}
              onMouseLeave={() => setHover(null)}
              onClick={() => onSelect?.(seg.row)}
              className="absolute top-0 bottom-0 rounded-[3px] overflow-hidden flex items-center
                         px-1.5 text-[10px] font-mono uppercase tracking-[0.04em] whitespace-nowrap
                         transition-all duration-150"
              style={{
                left: `${leftPct}%`,
                width: `${Math.max(widthPct, 0.2)}%`,
                background: isHovered
                  ? "var(--accent-hover, var(--accent))"
                  : "var(--accent)",
                color: "var(--accent-text, white)",
                minWidth: 0,
              }}
              title={tooltipFor(seg.row)}
              aria-label={tooltipFor(seg.row)}
            >
              <span className="truncate">{seg.row.issue_key || "?"}</span>
              {isHovered && (
                <BarTooltip leftPct={leftPct} widthPct={widthPct} row={seg.row} />
              )}
            </button>
          );
        })}
      </div>
      <div className="mt-2 text-[10px] text-[var(--text-tertiary)]">
        Klikněte na záznam pro zvýraznění v seznamu níže.
      </div>
    </div>
  );
}

function BarTooltip({
  row,
}: {
  leftPct: number;
  widthPct: number;
  row: WorklogRow;
}) {
  return (
    <div
      role="tooltip"
      className="absolute left-1/2 -translate-x-1/2 top-[calc(100%+4px)] z-20
                 px-2 py-1.5 rounded-[var(--radius-sm)] text-[11px] whitespace-nowrap
                 pointer-events-none"
      style={{
        background: "var(--bg-elevated)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      <div className="font-medium">{row.issue_key}</div>
      {row.summary && (
        <div className="text-[var(--text-tertiary)] max-w-[260px] truncate">
          {row.summary}
        </div>
      )}
      <div className="text-[var(--accent)] font-mono tabular-nums">
        {formatDurationShort(row.duration_s)}
      </div>
    </div>
  );
}

function tooltipFor(row: WorklogRow): string {
  return `${row.issue_key}${row.summary ? ` — ${row.summary}` : ""} (${formatDurationShort(row.duration_s)})`;
}

export function buildSegments(rows: WorklogRow[], day: Date): Segment[] {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const windowStartMs = dayStart.getTime() + START_HOUR * 3_600_000;
  const windowEndMs = dayStart.getTime() + END_HOUR * 3_600_000;

  const out: Segment[] = [];
  for (const r of rows) {
    const a = r.started_at * 1000;
    const b = a + r.duration_s * 1000;
    const clampA = Math.max(a, windowStartMs);
    const clampB = Math.min(b, windowEndMs);
    if (clampB <= clampA) continue;

    const leftFrac = (clampA - windowStartMs) / 3_600_000;
    const widthFrac = (clampB - clampA) / 3_600_000;
    out.push({ row: r, leftFrac, widthFrac });
  }
  // Sort by start so longer-overlapping ones don't cover shorter ones.
  out.sort((x, y) => x.leftFrac - y.leftFrac);
  return out;
}

/**
 * Phase 18A — Item 8: re-exported "bucketize" for the legacy hour-fill
 * computation; some older tests still consume it. The new view renders
 * `buildSegments` instead.
 */
export function bucketize(
  rows: WorklogRow[],
  day: Date,
): { hour: number; fill: number }[] {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const start = dayStart.getTime();
  const end = start + 86_400_000;

  const minutes = new Array<number>(END_HOUR - START_HOUR).fill(0);
  for (const r of rows) {
    const a = r.started_at * 1000;
    const b = a + r.duration_s * 1000;
    const clampA = Math.max(a, start);
    const clampB = Math.min(b, end);
    if (clampB <= clampA) continue;

    let cursor = clampA;
    while (cursor < clampB) {
      const d = new Date(cursor);
      const hour = d.getHours();
      const hourEnd = new Date(d);
      hourEnd.setMinutes(60, 0, 0);
      const slice = Math.min(clampB, hourEnd.getTime()) - cursor;
      if (hour >= START_HOUR && hour < END_HOUR) {
        minutes[hour - START_HOUR] = Math.min(
          60,
          minutes[hour - START_HOUR] + slice / 60_000,
        );
      }
      cursor += slice;
    }
  }
  return minutes.map((m, i) => ({ hour: START_HOUR + i, fill: m / 60 }));
}

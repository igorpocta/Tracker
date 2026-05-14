/**
 * Horizontal day timeline — accent-tinted blocks for each hour you logged.
 *
 * Reference: `screens/SCR-20260514-rjbm-2.png`.
 *
 *   06  07  08  09  10  11  12  13  14  15  16  17  18  19  20  21  22
 *  ────┴───┴───┴┬──┬───┴───┴───┴┬──┬───┴┬──┬───┴───┴───┴┬──┬───┴───┴
 *               │██│            │██│    │██│            │██│
 *
 * Each hour cell is sized in proportion to the share of that hour spent on
 * worklogs (overlapping logs cap at 100%). Cells with no activity render a
 * faint placeholder strip so the row remains rhythmically aligned.
 *
 * The width is computed at render time from the rows alone — no Layout
 * Effect, no resize observer. The container is responsive: the row of
 * 17 cells just shrinks/grows on its own.
 */
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";

/** First hour shown (inclusive). */
const START_HOUR = 6;
/** Last hour shown (exclusive). */
const END_HOUR = 22;

export interface DayTimelineProps {
  /** Worklog rows for the day. Each row must have `started_at`+`duration_s`. */
  rows: WorklogRow[];
  /** The day this timeline represents. Used to clamp ranges to its window. */
  day: Date;
}

interface HourBucket {
  hour: number;
  /** Fill ratio in [0, 1] — share of this hour covered by worklogs. */
  fill: number;
}

export function DayTimeline({ rows, day }: DayTimelineProps) {
  const buckets = useMemo(() => bucketize(rows, day), [rows, day]);

  return (
    <div
      className="rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] p-3"
      aria-label="Day timeline"
    >
      <h3 className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-3">
        Day timeline
      </h3>
      <div className="grid grid-cols-17 gap-1" style={{
        gridTemplateColumns: `repeat(${END_HOUR - START_HOUR}, minmax(0, 1fr))`,
      }}>
        {buckets.map((b) => (
          <HourCell key={b.hour} bucket={b} />
        ))}
      </div>
      <div
        className="mt-1 grid"
        style={{
          gridTemplateColumns: `repeat(${END_HOUR - START_HOUR}, minmax(0, 1fr))`,
        }}
      >
        {buckets.map((b) => (
          <div
            key={`label-${b.hour}`}
            className="text-[10px] font-mono text-[var(--text-tertiary)] text-left tabular-nums"
          >
            {b.hour}
          </div>
        ))}
      </div>
      <div className="mt-2 text-[10px] text-[var(--text-tertiary)]">
        Hold{" "}
        <kbd className="inline-block font-mono px-1 rounded bg-[var(--bg-hover)]">
          Shift
        </kbd>{" "}
        for more functions
      </div>
    </div>
  );
}

function HourCell({ bucket }: { bucket: HourBucket }) {
  const filled = bucket.fill > 0.02;
  return (
    <div
      className="relative h-9 rounded-[var(--radius-sm)] overflow-hidden"
      style={{
        background: filled ? "var(--accent-soft)" : "transparent",
        border: filled
          ? "1px solid var(--accent-soft)"
          : "1px solid var(--border-subtle)",
      }}
      title={`${bucket.hour}:00 — ${Math.round(bucket.fill * 60)}m`}
    >
      {filled && (
        <div
          aria-hidden
          className="absolute inset-0"
          style={{
            background: `linear-gradient(180deg, var(--accent-soft) 0%, transparent 100%)`,
            opacity: 0.6 + bucket.fill * 0.4,
          }}
        />
      )}
      <div
        aria-hidden
        className="absolute left-0 right-0 bottom-0"
        style={{
          height: `${Math.max(filled ? 30 : 0, bucket.fill * 100)}%`,
          background: "var(--accent)",
          opacity: filled ? 0.75 : 0,
        }}
      />
    </div>
  );
}

/**
 * Compute per-hour fill ratios in the day's local-time clock.
 *
 * For each worklog row we walk from `started_at` to `started_at + duration_s`,
 * clamp to the day's [START_HOUR, END_HOUR] window, and add the per-hour
 * coverage. Multiple overlapping logs in the same hour cap the fill at 1.0.
 */
export function bucketize(rows: WorklogRow[], day: Date): HourBucket[] {
  const dayStart = new Date(day);
  dayStart.setHours(0, 0, 0, 0);
  const start = dayStart.getTime();
  const end = start + 86_400_000;

  const buckets: HourBucket[] = [];
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
      // Compute the end-of-hour boundary in ms.
      const hourEnd = new Date(d);
      hourEnd.setMinutes(60, 0, 0);
      const slice = Math.min(clampB, hourEnd.getTime()) - cursor;
      if (hour >= START_HOUR && hour < END_HOUR) {
        minutes[hour - START_HOUR] = Math.min(60, minutes[hour - START_HOUR] + slice / 60_000);
      }
      cursor += slice;
    }
  }

  for (let i = 0; i < minutes.length; i++) {
    buckets.push({ hour: START_HOUR + i, fill: minutes[i] / 60 });
  }
  return buckets;
}

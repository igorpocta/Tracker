/**
 * Unit tests for the day-timeline `bucketize` helper.
 *
 * The function distributes worklog rows into per-hour buckets within the
 * 06:00–22:00 window. We assert:
 *   - Empty input yields all zero fills.
 *   - A 1h worklog at 10:00 yields 100% fill in hour 10 and 0 elsewhere.
 *   - A worklog spanning two hours splits proportionally.
 *   - Rows outside the displayed window are clamped away.
 */
import { describe, expect, it } from "vitest";

import type { WorklogRow } from "../../api/types";
import { bucketize, canvasXToTimeMs, formatRangeLabel } from "./DayTimeline";

function mkRow(startISO: string, durationSeconds: number): WorklogRow {
  return {
    issue_key: "DEV-1",
    duration_s: durationSeconds,
    started_at: Math.floor(new Date(startISO).getTime() / 1000),
    logged_at: 0,
  };
}

describe("bucketize", () => {
  const day = new Date("2026-05-14T00:00:00");

  it("returns all zero buckets for an empty row list", () => {
    const buckets = bucketize([], day);
    expect(buckets).toHaveLength(22 - 6);
    expect(buckets.every((b) => b.fill === 0)).toBe(true);
  });

  it("fills a single hour completely when given a 60-minute log", () => {
    const rows = [mkRow("2026-05-14T10:00:00", 60 * 60)];
    const buckets = bucketize(rows, day);
    const ten = buckets.find((b) => b.hour === 10)!;
    expect(ten.fill).toBeCloseTo(1, 5);
    // Adjacent hours stay empty.
    expect(buckets.find((b) => b.hour === 9)!.fill).toBe(0);
    expect(buckets.find((b) => b.hour === 11)!.fill).toBe(0);
  });

  it("splits across hours for logs that span boundaries", () => {
    // 11:30 → 12:30, 60 minutes total.
    const rows = [mkRow("2026-05-14T11:30:00", 60 * 60)];
    const buckets = bucketize(rows, day);
    expect(buckets.find((b) => b.hour === 11)!.fill).toBeCloseTo(0.5, 2);
    expect(buckets.find((b) => b.hour === 12)!.fill).toBeCloseTo(0.5, 2);
  });

  it("clamps rows that fall outside the 06–22 window", () => {
    const rows = [mkRow("2026-05-14T02:00:00", 60 * 60)];
    const buckets = bucketize(rows, day);
    expect(buckets.every((b) => b.fill === 0)).toBe(true);
  });

  it("caps overlapping coverage at a single hour's max fill", () => {
    const rows = [
      mkRow("2026-05-14T14:00:00", 45 * 60),
      mkRow("2026-05-14T14:15:00", 45 * 60),
    ];
    const buckets = bucketize(rows, day);
    const fourteen = buckets.find((b) => b.hour === 14)!;
    expect(fourteen.fill).toBeLessThanOrEqual(1);
    expect(fourteen.fill).toBeGreaterThan(0.7);
  });
});

describe("canvasXToTimeMs", () => {
  const day = new Date("2026-05-14T00:00:00");

  it("always returns an integer ms timestamp", () => {
    // The Tauri commands `create_manual_worklog` and `split_worklog` are
    // typed `started_at_ms: i64` / `split_at_ms: i64`. Serde rejects floats
    // (`invalid type: floating point '...', expected i64`). The pixel→time
    // math is inherently floating-point, so the function must round before
    // returning. Regression for the 2026-05-15 crash on timeline-drag
    // create.
    const cssWidth = 731;
    // Pick a deliberately ugly x that yields a fractional ms when
    // multiplied through `frac * (END_HOUR - START_HOUR) * 3_600_000`.
    for (const x of [0, 1, 17, 113, 365, 729, 730, 731]) {
      const ms = canvasXToTimeMs(x, cssWidth, day);
      expect(Number.isInteger(ms)).toBe(true);
    }
  });

  it("maps x=0 to the start-of-window timestamp", () => {
    const ms = canvasXToTimeMs(0, 1000, day);
    const expected = new Date("2026-05-14T06:00:00").getTime();
    expect(ms).toBe(expected);
  });

  it("clamps canvasX > cssWidth to the end-of-window timestamp", () => {
    const ms = canvasXToTimeMs(2000, 1000, day);
    const expected = new Date("2026-05-14T22:00:00").getTime();
    expect(ms).toBe(expected);
  });

  it("clamps negative canvasX to the start-of-window timestamp", () => {
    const ms = canvasXToTimeMs(-50, 1000, day);
    const expected = new Date("2026-05-14T06:00:00").getTime();
    expect(ms).toBe(expected);
  });
});

describe("formatRangeLabel", () => {
  const at = (h: number, m: number) =>
    new Date(2026, 4, 14, h, m, 0, 0).getTime();

  it("shows start, end and duration for a drag range", () => {
    expect(formatRangeLabel(at(15, 10), at(15, 40))).toBe(
      "15:10 – 15:40 · 30m",
    );
  });

  it("formats multi-hour durations", () => {
    expect(formatRangeLabel(at(9, 0), at(10, 30))).toBe("09:00 – 10:30 · 1h 30m");
  });

  it("normalises reversed bounds (drag right-to-left)", () => {
    expect(formatRangeLabel(at(15, 40), at(15, 10))).toBe(
      "15:10 – 15:40 · 30m",
    );
  });
});

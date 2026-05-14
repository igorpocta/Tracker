/**
 * Tests for `buildSegments` — Phase 18B Item 31.
 */
import { describe, expect, it } from "vitest";

import type { WorklogRow } from "../../api/types";
import { buildSegments } from "./DayTimeline";

function mkRow(start: Date, durationSeconds: number, key = "DEV-1"): WorklogRow {
  return {
    issue_key: key,
    duration_s: durationSeconds,
    started_at: Math.floor(start.getTime() / 1000),
    logged_at: 0,
  };
}

describe("buildSegments", () => {
  const day = new Date(2026, 4, 14);

  it("returns one segment per row, sorted by start", () => {
    const rows = [
      mkRow(new Date(2026, 4, 14, 14, 0), 60 * 60, "DEV-2"),
      mkRow(new Date(2026, 4, 14, 8, 30), 30 * 60, "DEV-1"),
    ];
    const segs = buildSegments(rows, day);
    expect(segs.map((s) => s.row.issue_key)).toEqual(["DEV-1", "DEV-2"]);
    // 8:30 → 06+2.5 hours offset.
    expect(segs[0].leftFrac).toBeCloseTo(2.5, 2);
    expect(segs[0].widthFrac).toBeCloseTo(0.5, 2);
  });

  it("clamps rows fully outside 06–22", () => {
    const rows = [mkRow(new Date(2026, 4, 14, 2, 0), 60 * 60)];
    const segs = buildSegments(rows, day);
    expect(segs).toHaveLength(0);
  });

  it("clamps partial overflows to the window", () => {
    // Started at 21:30, runs 90 minutes → should clamp to a 0.5h bar.
    const rows = [mkRow(new Date(2026, 4, 14, 21, 30), 90 * 60)];
    const segs = buildSegments(rows, day);
    expect(segs).toHaveLength(1);
    expect(segs[0].leftFrac).toBeCloseTo(15.5, 2);
    expect(segs[0].widthFrac).toBeCloseTo(0.5, 2);
  });
});

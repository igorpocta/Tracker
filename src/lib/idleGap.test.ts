import { describe, expect, it } from "vitest";

import { clampDiscardStartMs } from "./idleGap";

describe("clampDiscardStartMs", () => {
  it("shifts the start forward by the idle gap in the normal case", () => {
    // started 10:00, idle 30m, now 11:00 -> new start 10:30 (30m worked before).
    const start = 10 * 3600_000;
    const idle = 30 * 60_000;
    const now = 11 * 3600_000;
    expect(clampDiscardStartMs(start, idle, now)).toBe(start + idle);
  });

  it("never moves the start past now (no negative/zero-corrupting duration)", () => {
    // Pathological: idle gap reported longer than total elapsed.
    const start = 10 * 3600_000;
    const now = start + 5 * 60_000; // only 5m elapsed
    const idle = 60 * 60_000; // but 60m "idle"
    expect(clampDiscardStartMs(start, idle, now)).toBe(now);
  });

  it("never moves the start before its original value", () => {
    const start = 10 * 3600_000;
    expect(clampDiscardStartMs(start, -5000, start + 10_000)).toBe(start);
  });
});

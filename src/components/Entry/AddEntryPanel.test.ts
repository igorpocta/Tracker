/**
 * Tests for the duration math used by the Add entry panel.
 */
import { describe, expect, it } from "vitest";

import { computeDurationMinutes, crossesMidnight } from "./AddEntryPanel";

describe("computeDurationMinutes", () => {
  it("returns the difference for a valid window", () => {
    expect(computeDurationMinutes("09:00", "10:30")).toBe(90);
    expect(computeDurationMinutes("15:00", "15:15")).toBe(15);
  });

  it("returns 0 when start equals end", () => {
    expect(computeDurationMinutes("10:00", "10:00")).toBe(0);
  });

  it("wraps past midnight when end is strictly before start", () => {
    // Regression: 23:30 + 1h used to land at 00:30 and computeDurationMinutes
    // returned 0 → the save button stayed disabled. Now it returns 60.
    expect(computeDurationMinutes("23:30", "00:30")).toBe(60);
    // Symmetric edge: 22:00 → 02:00 is a 4h entry.
    expect(computeDurationMinutes("22:00", "02:00")).toBe(4 * 60);
    // Longest representable wrapped interval: 00:01 → 00:00 is 23h59m.
    expect(computeDurationMinutes("00:01", "00:00")).toBe(23 * 60 + 59);
  });

  it("handles HH:MM zero-padding tolerantly", () => {
    expect(computeDurationMinutes("9:00", "10:00")).toBe(60);
  });

  it("returns 0 for malformed input", () => {
    expect(computeDurationMinutes("9 AM", "10 AM")).toBe(0);
    expect(computeDurationMinutes("", "")).toBe(0);
  });
});

describe("crossesMidnight", () => {
  it("is true only when end is strictly less than start", () => {
    expect(crossesMidnight("23:30", "00:30")).toBe(true);
    expect(crossesMidnight("00:00", "23:59")).toBe(false);
    expect(crossesMidnight("10:00", "10:00")).toBe(false);
  });

  it("returns false for malformed input", () => {
    expect(crossesMidnight("nope", "00:30")).toBe(false);
    expect(crossesMidnight("23:30", "nope")).toBe(false);
  });
});

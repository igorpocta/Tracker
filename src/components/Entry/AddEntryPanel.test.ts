/**
 * Tests for the duration math used by the Add entry panel.
 */
import { describe, expect, it } from "vitest";

import { computeDurationMinutes } from "./AddEntryPanel";

describe("computeDurationMinutes", () => {
  it("returns the difference for a valid window", () => {
    expect(computeDurationMinutes("09:00", "10:30")).toBe(90);
    expect(computeDurationMinutes("15:00", "15:15")).toBe(15);
  });

  it("clamps to 0 when end is before start", () => {
    expect(computeDurationMinutes("10:00", "09:00")).toBe(0);
    expect(computeDurationMinutes("10:00", "10:00")).toBe(0);
  });

  it("handles HH:MM zero-padding tolerantly", () => {
    expect(computeDurationMinutes("9:00", "10:00")).toBe(60);
  });

  it("returns 0 for malformed input", () => {
    expect(computeDurationMinutes("9 AM", "10 AM")).toBe(0);
    expect(computeDurationMinutes("", "")).toBe(0);
  });
});

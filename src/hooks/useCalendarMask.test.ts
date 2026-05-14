/**
 * Pure-function tests for `isWorkingDayLocal` (Phase 18B Item 1).
 */
import { describe, expect, it } from "vitest";

import { isWorkingDayLocal } from "./useCalendarMask";

describe("isWorkingDayLocal", () => {
  const MONFRI = 0b0011111; // Mon..Fri (Mon = bit 0).

  it("treats Mon–Fri as working with the default mask", () => {
    // 2026-05-11 is a Monday.
    const mon = new Date(2026, 4, 11);
    const tue = new Date(2026, 4, 12);
    const fri = new Date(2026, 4, 15);
    const sat = new Date(2026, 4, 16);
    const sun = new Date(2026, 4, 17);
    expect(isWorkingDayLocal(mon, MONFRI, new Set())).toBe(true);
    expect(isWorkingDayLocal(tue, MONFRI, new Set())).toBe(true);
    expect(isWorkingDayLocal(fri, MONFRI, new Set())).toBe(true);
    expect(isWorkingDayLocal(sat, MONFRI, new Set())).toBe(false);
    expect(isWorkingDayLocal(sun, MONFRI, new Set())).toBe(false);
  });

  it("excludes dates in the non-working-day set", () => {
    const mon = new Date(2026, 4, 11);
    expect(
      isWorkingDayLocal(mon, MONFRI, new Set(["2026-05-11"])),
    ).toBe(false);
  });

  it("respects a custom mask (e.g. Saturday work week)", () => {
    // Mask includes Sat (bit 5) but not Mon.
    const mask = 0b0100000 | 0b0011110; // Tue..Fri + Sat
    const mon = new Date(2026, 4, 11);
    const sat = new Date(2026, 4, 16);
    expect(isWorkingDayLocal(mon, mask, new Set())).toBe(false);
    expect(isWorkingDayLocal(sat, mask, new Set())).toBe(true);
  });
});

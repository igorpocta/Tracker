/**
 * Unit tests for the working-day arithmetic used by the Goals view.
 */
import { describe, expect, it } from "vitest";

import { countWorkingDays } from "./Goals";

describe("countWorkingDays", () => {
  it("counts Mon-Fri only", () => {
    // 2026-05-04 is a Monday. 2026-05-08 is a Friday.
    const from = new Date("2026-05-04T00:00:00");
    const to = new Date("2026-05-08T00:00:00");
    expect(countWorkingDays(from, to)).toBe(5);
  });

  it("excludes weekends", () => {
    // Sat (5/9) + Sun (5/10) → 0 working days.
    expect(
      countWorkingDays(
        new Date("2026-05-09T00:00:00"),
        new Date("2026-05-10T00:00:00"),
      ),
    ).toBe(0);
  });

  it("works for a full month with mixed weekdays/weekends", () => {
    // May 2026: 1st = Friday, 31st = Sunday. 21 weekdays.
    expect(
      countWorkingDays(
        new Date("2026-05-01T00:00:00"),
        new Date("2026-05-31T00:00:00"),
      ),
    ).toBe(21);
  });

  it("returns 0 when to < from", () => {
    expect(
      countWorkingDays(
        new Date("2026-05-10T00:00:00"),
        new Date("2026-05-01T00:00:00"),
      ),
    ).toBe(0);
  });
});

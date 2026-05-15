/**
 * Tests for the Zod schemas + `parseRateInput` helper (Phase 18C — Item 23).
 *
 * Focus: cases that previously crashed the app — `2e99`, `-1`, NaN, blank
 * input, lowercase issue keys, NUL-injected JQL, etc.
 */
import { describe, expect, it } from "vitest";

import {
  activityThresholdSchema,
  currencySchema,
  dailyGoalHoursSchema,
  firstError,
  goalSliderHoursSchema,
  hourlyRateSchema,
  issueKeySchema,
  jqlSchema,
  parseRateInput,
  roundingIntervalSchema,
  timeOfDaySchema,
  workingWeekMaskSchema,
} from "./validation";

describe("parseRateInput", () => {
  it("returns 0 for blank input", () => {
    expect(parseRateInput("")).toBe(0);
    expect(parseRateInput("   ")).toBe(0);
  });

  it("parses plain decimals (dot or comma)", () => {
    expect(parseRateInput("1500")).toBe(1500);
    expect(parseRateInput("1500.5")).toBe(1500.5);
    expect(parseRateInput("1500,75")).toBe(1500.75);
  });

  it("rejects scientific notation outright", () => {
    expect(parseRateInput("2e99")).toBeNull();
    expect(parseRateInput("1E10")).toBeNull();
    expect(parseRateInput("1e0")).toBeNull();
  });

  it("rejects non-numeric junk", () => {
    expect(parseRateInput("abc")).toBeNull();
    expect(parseRateInput("100kč")).toBeNull();
    expect(parseRateInput("--5")).toBeNull();
    expect(parseRateInput("1.2.3")).toBeNull();
  });

  it("preserves negative sign — caller decides if it's valid", () => {
    // The function only parses; the schema rejects negatives.
    expect(parseRateInput("-1")).toBe(-1);
    expect(firstError(hourlyRateSchema, parseRateInput("-1"))).not.toBeNull();
  });
});

describe("hourlyRateSchema", () => {
  it("accepts the canonical range", () => {
    expect(firstError(hourlyRateSchema, 0)).toBeNull();
    expect(firstError(hourlyRateSchema, 1500)).toBeNull();
    expect(firstError(hourlyRateSchema, 99_999)).toBeNull();
  });

  it("rejects infinity / NaN", () => {
    expect(firstError(hourlyRateSchema, Number.POSITIVE_INFINITY)).not.toBeNull();
    expect(firstError(hourlyRateSchema, Number.NEGATIVE_INFINITY)).not.toBeNull();
    expect(firstError(hourlyRateSchema, NaN)).not.toBeNull();
  });

  it("rejects negative + overlarge", () => {
    expect(firstError(hourlyRateSchema, -0.01)).not.toBeNull();
    expect(firstError(hourlyRateSchema, 100_000)).not.toBeNull();
    expect(firstError(hourlyRateSchema, 1e10)).not.toBeNull();
  });
});

describe("dailyGoalHoursSchema", () => {
  it("accepts 0.5–24 h", () => {
    expect(firstError(dailyGoalHoursSchema, 0.5)).toBeNull();
    expect(firstError(dailyGoalHoursSchema, 8)).toBeNull();
    expect(firstError(dailyGoalHoursSchema, 24)).toBeNull();
  });

  it("rejects out-of-range", () => {
    expect(firstError(dailyGoalHoursSchema, 0)).not.toBeNull();
    expect(firstError(dailyGoalHoursSchema, 0.1)).not.toBeNull();
    expect(firstError(dailyGoalHoursSchema, 25)).not.toBeNull();
    expect(firstError(dailyGoalHoursSchema, NaN)).not.toBeNull();
  });
});

describe("activityThresholdSchema", () => {
  it("requires an integer in [1, 120]", () => {
    expect(firstError(activityThresholdSchema, 1)).toBeNull();
    expect(firstError(activityThresholdSchema, 60)).toBeNull();
    expect(firstError(activityThresholdSchema, 120)).toBeNull();
    expect(firstError(activityThresholdSchema, 0)).not.toBeNull();
    expect(firstError(activityThresholdSchema, 121)).not.toBeNull();
    expect(firstError(activityThresholdSchema, 5.5)).not.toBeNull();
  });
});

describe("roundingIntervalSchema", () => {
  it("accepts only the canonical set", () => {
    for (const n of [1, 5, 15, 60]) {
      expect(firstError(roundingIntervalSchema, n)).toBeNull();
    }
  });
  it("rejects everything else", () => {
    for (const n of [0, 2, 10, 30, 90, 120, -1]) {
      expect(firstError(roundingIntervalSchema, n)).not.toBeNull();
    }
  });
});

describe("workingWeekMaskSchema", () => {
  it("accepts 0..=127 integers", () => {
    expect(firstError(workingWeekMaskSchema, 0)).toBeNull();
    expect(firstError(workingWeekMaskSchema, 31)).toBeNull(); // Mon-Fri
    expect(firstError(workingWeekMaskSchema, 127)).toBeNull();
  });
  it("rejects out-of-range / non-integers", () => {
    expect(firstError(workingWeekMaskSchema, -1)).not.toBeNull();
    expect(firstError(workingWeekMaskSchema, 128)).not.toBeNull();
    expect(firstError(workingWeekMaskSchema, 1.5)).not.toBeNull();
  });
});

describe("goalSliderHoursSchema", () => {
  it("accepts 1..=14", () => {
    expect(firstError(goalSliderHoursSchema, 1)).toBeNull();
    expect(firstError(goalSliderHoursSchema, 14)).toBeNull();
    expect(firstError(goalSliderHoursSchema, 8.5)).toBeNull();
  });
  it("rejects outside", () => {
    expect(firstError(goalSliderHoursSchema, 0.99)).not.toBeNull();
    expect(firstError(goalSliderHoursSchema, 14.5)).not.toBeNull();
  });
});

describe("timeOfDaySchema", () => {
  it("accepts HH:MM", () => {
    expect(firstError(timeOfDaySchema, "00:00")).toBeNull();
    expect(firstError(timeOfDaySchema, "9:30")).toBeNull();
    expect(firstError(timeOfDaySchema, "23:59")).toBeNull();
  });
  it("rejects bad shapes", () => {
    expect(firstError(timeOfDaySchema, "24:00")).not.toBeNull();
    expect(firstError(timeOfDaySchema, "12:60")).not.toBeNull();
    expect(firstError(timeOfDaySchema, "1230")).not.toBeNull();
    expect(firstError(timeOfDaySchema, "")).not.toBeNull();
    expect(firstError(timeOfDaySchema, "noon")).not.toBeNull();
  });
});

describe("issueKeySchema", () => {
  it("accepts canonical keys", () => {
    expect(firstError(issueKeySchema, "ACME-1")).toBeNull();
    expect(firstError(issueKeySchema, "PROJ-12345")).toBeNull();
    expect(firstError(issueKeySchema, "AB1-99")).toBeNull();
  });
  it("rejects malformed keys", () => {
    expect(firstError(issueKeySchema, "acme-1")).not.toBeNull();
    expect(firstError(issueKeySchema, "ACME-01")).not.toBeNull();
    expect(firstError(issueKeySchema, "A-1")).not.toBeNull();
    expect(firstError(issueKeySchema, "ACME 1")).not.toBeNull();
    expect(firstError(issueKeySchema, "")).not.toBeNull();
  });
});

describe("jqlSchema", () => {
  it("accepts typical queries", () => {
    expect(firstError(jqlSchema, "project = ACME")).toBeNull();
    expect(firstError(jqlSchema, "assignee = currentUser()")).toBeNull();
  });
  it("rejects blank / NUL / overlong", () => {
    expect(firstError(jqlSchema, "")).not.toBeNull();
    expect(firstError(jqlSchema, "   ")).not.toBeNull();
    expect(firstError(jqlSchema, "abc\0def")).not.toBeNull();
    expect(firstError(jqlSchema, "x".repeat(2001))).not.toBeNull();
  });
});

describe("currencySchema", () => {
  it("accepts the allowed list", () => {
    for (const c of ["CZK", "EUR", "USD", "GBP", "PLN", "CHF"] as const) {
      expect(firstError(currencySchema, c)).toBeNull();
    }
  });
  it("rejects others", () => {
    expect(firstError(currencySchema, "JPY")).not.toBeNull();
    expect(firstError(currencySchema, "czk")).not.toBeNull();
    expect(firstError(currencySchema, "")).not.toBeNull();
  });
});

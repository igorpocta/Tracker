import { describe, expect, it } from "vitest";

import {
  formatClockTime,
  formatDuration,
  formatDurationShort,
  formatHours,
  formatMoney,
  formatRelativeTime,
  formatWeekdayCs,
  formatWeekdayShort,
  isToday,
  pluralCs,
} from "./format";

describe("formatDuration", () => {
  it("formats whole seconds as HH:MM:SS", () => {
    expect(formatDuration(0)).toBe("00:00:00");
    expect(formatDuration(59)).toBe("00:00:59");
    expect(formatDuration(60)).toBe("00:01:00");
    expect(formatDuration(3600)).toBe("01:00:00");
    expect(formatDuration(3661)).toBe("01:01:01");
  });

  it("handles >24h durations without truncating hours", () => {
    expect(formatDuration(25 * 3600)).toBe("25:00:00");
  });

  it("clamps negative or non-finite input", () => {
    expect(formatDuration(-5)).toBe("00:00:00");
    expect(formatDuration(NaN)).toBe("00:00:00");
    expect(formatDuration(Infinity)).toBe("00:00:00");
  });

  it("floors fractional seconds", () => {
    expect(formatDuration(59.9)).toBe("00:00:59");
  });
});

describe("formatDurationShort", () => {
  it("returns 0m for empty input", () => {
    expect(formatDurationShort(0)).toBe("0m");
    expect(formatDurationShort(-1)).toBe("0m");
  });

  it("under a minute -> seconds", () => {
    expect(formatDurationShort(45)).toBe("45s");
  });

  it("under an hour -> minutes only", () => {
    expect(formatDurationShort(60)).toBe("1m");
    expect(formatDurationShort(15 * 60)).toBe("15m");
  });

  it("hours and minutes combine", () => {
    expect(formatDurationShort(3600)).toBe("1h");
    expect(formatDurationShort(3600 + 15 * 60)).toBe("1h 15m");
  });
});

describe("formatHours", () => {
  it("returns `Nh` for whole hours and one decimal otherwise", () => {
    expect(formatHours(0)).toBe("0h");
    expect(formatHours(1)).toBe("1h");
    expect(formatHours(7.5)).toBe("7.5h");
    expect(formatHours(7.55)).toBe("7.6h");
  });
});

describe("formatRelativeTime", () => {
  it("reports 'právě teď' for very small diffs", () => {
    const now = new Date(2024, 0, 1, 12, 0, 0);
    expect(formatRelativeTime(now.getTime() - 5_000, now)).toBe("právě teď");
  });

  it("reports minutes, hours, days in Czech", () => {
    const now = new Date(2024, 0, 10, 12, 0, 0);
    expect(formatRelativeTime(now.getTime() - 5 * 60_000, now)).toBe("před 5 min");
    expect(formatRelativeTime(now.getTime() - 3 * 3_600_000, now)).toBe("před 3 h");
    expect(formatRelativeTime(now.getTime() - 2 * 86_400_000, now)).toBe("před 2 dny");
  });

  it("renders English when lang='en'", () => {
    const now = new Date("2026-07-01T12:00:00Z");
    expect(formatRelativeTime(now.getTime() - 5_000, now, "en")).toBe("just now");
    expect(formatRelativeTime(now.getTime() - 5 * 60_000, now, "en")).toBe("5 min ago");
    expect(formatRelativeTime(now.getTime() - 2 * 86_400_000, now, "en")).toBe("2 d ago");
  });

  it("weekday names follow the language", () => {
    const wed = new Date(2026, 6, 1); // Wednesday
    expect(formatWeekdayCs(wed, "cs")).toBe("Středa");
    expect(formatWeekdayCs(wed, "en")).toBe("Wednesday");
    expect(formatWeekdayShort(3, "cs")).toBe("St");
    expect(formatWeekdayShort(3, "en")).toBe("We");
  });

  it("accepts unix seconds", () => {
    const now = new Date(2024, 0, 10, 12, 0, 0);
    const secs = Math.floor((now.getTime() - 10 * 60_000) / 1000);
    expect(formatRelativeTime(secs, now)).toBe("před 10 min");
  });
});

describe("formatClockTime", () => {
  it("pads HH and MM", () => {
    const d = new Date(2024, 0, 1, 9, 5, 0);
    expect(formatClockTime(d)).toBe("09:05");
  });
});

describe("formatMoney", () => {
  // CZK uses thin-space grouping + "Kč" suffix, rounded to whole units.
  const thin = " ";

  it("formats CZK with thin-space groups and Kč suffix", () => {
    expect(formatMoney(1234, "CZK")).toBe(`1${thin}234${thin}Kč`);
    expect(formatMoney(0, "CZK")).toBe(`0${thin}Kč`);
    expect(formatMoney(1234567.4, "CZK")).toBe(`1${thin}234${thin}567${thin}Kč`);
  });

  it("formats EUR with euro prefix and two decimals", () => {
    expect(formatMoney(42.5, "EUR")).toBe("€42.50");
    expect(formatMoney(1234.56, "EUR")).toBe("€1,234.56");
  });

  it("formats USD with dollar prefix", () => {
    expect(formatMoney(1234.5, "USD")).toBe("$1,234.50");
  });

  it("formats GBP with pound prefix", () => {
    expect(formatMoney(7.25, "GBP")).toBe("£7.25");
  });

  it("formats PLN with thin-space + zł suffix", () => {
    expect(formatMoney(150, "PLN")).toBe(`150${thin}zł`);
  });

  it("falls back to ISO code suffix for unknown currencies", () => {
    expect(formatMoney(10, "JPY")).toBe(`10.00${thin}JPY`);
  });

  it("handles non-finite gracefully", () => {
    expect(formatMoney(NaN, "EUR")).toBe("—");
    expect(formatMoney(Infinity, "USD")).toBe("—");
  });
});

describe("isToday", () => {
  it("returns true for the same calendar day", () => {
    const now = new Date(2024, 4, 14, 18, 0, 0);
    const earlier = new Date(2024, 4, 14, 8, 0, 0).getTime() / 1000;
    expect(isToday(earlier, now)).toBe(true);
  });

  it("returns false across midnight", () => {
    const now = new Date(2024, 4, 14, 0, 0, 0);
    const yesterday = new Date(2024, 4, 13, 23, 30, 0).getTime() / 1000;
    expect(isToday(yesterday, now)).toBe(false);
  });
});

describe("pluralCs", () => {
  const forms: [string, string, string] = ["záznam", "záznamy", "záznamů"];

  it("uses the singular form for exactly 1", () => {
    expect(pluralCs(1, forms)).toBe("záznam");
  });

  it("uses the 2–4 form for 2, 3 and 4", () => {
    expect(pluralCs(2, forms)).toBe("záznamy");
    expect(pluralCs(3, forms)).toBe("záznamy");
    expect(pluralCs(4, forms)).toBe("záznamy");
  });

  it("uses the genitive-plural form for 0 and 5+", () => {
    expect(pluralCs(0, forms)).toBe("záznamů");
    expect(pluralCs(5, forms)).toBe("záznamů");
    expect(pluralCs(11, forms)).toBe("záznamů");
    expect(pluralCs(25, forms)).toBe("záznamů");
  });
});

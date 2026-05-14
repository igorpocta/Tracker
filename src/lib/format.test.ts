import { describe, expect, it } from "vitest";

import {
  formatClockTime,
  formatDuration,
  formatDurationShort,
  formatHours,
  formatRelativeTime,
  isToday,
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
  it("reports just now for very small diffs", () => {
    const now = new Date(2024, 0, 1, 12, 0, 0);
    expect(formatRelativeTime(now.getTime() - 5_000, now)).toBe("just now");
  });

  it("reports minutes, hours, days", () => {
    const now = new Date(2024, 0, 10, 12, 0, 0);
    expect(formatRelativeTime(now.getTime() - 5 * 60_000, now)).toBe("5m ago");
    expect(formatRelativeTime(now.getTime() - 3 * 3_600_000, now)).toBe("3h ago");
    expect(formatRelativeTime(now.getTime() - 2 * 86_400_000, now)).toBe("2d ago");
  });

  it("accepts unix seconds", () => {
    const now = new Date(2024, 0, 10, 12, 0, 0);
    const secs = Math.floor((now.getTime() - 10 * 60_000) / 1000);
    expect(formatRelativeTime(secs, now)).toBe("10m ago");
  });
});

describe("formatClockTime", () => {
  it("pads HH and MM", () => {
    const d = new Date(2024, 0, 1, 9, 5, 0);
    expect(formatClockTime(d)).toBe("09:05");
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

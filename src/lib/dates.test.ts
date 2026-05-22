import { describe, expect, it } from "vitest";

import {
  addDays,
  combineDateAndTimeAllowingNextDay,
  dayEndUnixS,
  dayRangeUnixS,
  dayStartUnixS,
  daysBetween,
  endOfMonth,
  formatIsoDate,
  formatShortDayLabel,
  isSameDay,
  lastNDays,
  startOfMonth,
  startOfPreviousMonth,
  startOfWeek,
  weekDays,
} from "./dates";

describe("dates", () => {
  it("dayStart/EndUnixS bracket the calendar day", () => {
    const d = new Date(2026, 4, 14, 15, 30); // 2026-05-14 15:30
    const start = dayStartUnixS(d);
    const end = dayEndUnixS(d);
    expect(end - start).toBe(86399); // 23:59:59 from 00:00:00
    const startDate = new Date(start * 1000);
    expect(startDate.getHours()).toBe(0);
    expect(startDate.getMinutes()).toBe(0);
  });

  it("dayRangeUnixS returns [start, end]", () => {
    const d = new Date(2026, 0, 1);
    const [from, to] = dayRangeUnixS(d);
    expect(to).toBeGreaterThan(from);
    expect(to - from).toBe(86399);
  });

  it("addDays moves forward and backward", () => {
    const d = new Date(2026, 4, 14);
    expect(addDays(d, 1).getDate()).toBe(15);
    expect(addDays(d, -1).getDate()).toBe(13);
  });

  it("isSameDay ignores time of day", () => {
    const a = new Date(2026, 4, 14, 1, 0);
    const b = new Date(2026, 4, 14, 23, 30);
    expect(isSameDay(a, b)).toBe(true);
    expect(isSameDay(a, addDays(b, 1))).toBe(false);
  });

  it("startOfWeek returns the Monday", () => {
    // 2026-05-14 is a Thursday.
    const thursday = new Date(2026, 4, 14);
    const monday = startOfWeek(thursday);
    expect(monday.getDay()).toBe(1); // 1 = Monday
    expect(monday.getDate()).toBe(11);
  });

  it("weekDays produces 7 consecutive days", () => {
    const week = weekDays(new Date(2026, 4, 14));
    expect(week).toHaveLength(7);
    expect(week[0].getDay()).toBe(1);
    expect(week[6].getDay()).toBe(0); // Sunday is 0
  });

  it("lastNDays goes backwards starting from today", () => {
    const today = new Date(2026, 4, 14);
    const days = lastNDays(today, 3);
    expect(days[0].getDate()).toBe(14);
    expect(days[1].getDate()).toBe(13);
    expect(days[2].getDate()).toBe(12);
  });

  it("startOfMonth and endOfMonth", () => {
    const d = new Date(2026, 4, 14);
    expect(startOfMonth(d).getDate()).toBe(1);
    expect(endOfMonth(d).getDate()).toBe(31);
  });

  it("startOfPreviousMonth", () => {
    const d = new Date(2026, 4, 14);
    expect(startOfPreviousMonth(d).getMonth()).toBe(3); // April
    expect(startOfPreviousMonth(d).getDate()).toBe(1);
  });

  it("formatIsoDate uses YYYY-MM-DD", () => {
    expect(formatIsoDate(new Date(2026, 0, 5))).toBe("2026-01-05");
  });

  it("formatIsoDate is timezone-aware (local, not UTC)", () => {
    // Regression: SuggestionBanner previously used
    // `new Date().toISOString().slice(0, 10)` for its "Skrýt pro
    // dnešek" dismiss key, which is UTC. East of UTC the user's
    // dismiss lingered past their local midnight; west of UTC it
    // expired early. `formatIsoDate` instead reads `getFullYear`
    // / `getMonth` / `getDate` (all local), so the output matches
    // the user's wall-clock day even at the very edges of it.
    const late = new Date(2026, 4, 15, 23, 30, 0); // 23:30 LOCAL
    expect(formatIsoDate(late)).toBe("2026-05-15");
    // Sanity: the same instant via `toISOString()` will diverge for
    // any non-UTC timezone — we don't assert the exact UTC value
    // because the test runner's TZ is environment-dependent, but we
    // assert formatIsoDate stays anchored on the LOCAL day.
    expect(late.getDate()).toBe(15);
  });

  it("formatShortDayLabel uses Mon D.M form", () => {
    const out = formatShortDayLabel(new Date(2026, 4, 14));
    expect(out).toMatch(/\d+\.\d+/);
  });

  it("daysBetween counts whole days", () => {
    const a = new Date(2026, 0, 1);
    const b = new Date(2026, 0, 8);
    expect(daysBetween(a, b)).toBe(7);
    expect(daysBetween(b, a)).toBe(-7);
  });
});

describe("combineDateAndTimeAllowingNextDay", () => {
  it("returns null for invalid time string", () => {
    const base = new Date(2026, 4, 14, 23, 30, 0); // 2026-05-14 23:30 local
    expect(combineDateAndTimeAllowingNextDay(base, "x:y")).toBeNull();
  });

  it("uses the same day when the time is after the base time", () => {
    const base = new Date(2026, 4, 14, 9, 0, 0);
    const out = combineDateAndTimeAllowingNextDay(base, "17:30")!;
    expect(out.getFullYear()).toBe(2026);
    expect(out.getMonth()).toBe(4);
    expect(out.getDate()).toBe(14);
    expect(out.getHours()).toBe(17);
    expect(out.getMinutes()).toBe(30);
  });

  it("rolls to the next day when the time is BEFORE the base time", () => {
    // base = 2026-05-14 23:30; end = 00:30 → must be 2026-05-15 00:30
    const base = new Date(2026, 4, 14, 23, 30, 0);
    const out = combineDateAndTimeAllowingNextDay(base, "00:30")!;
    expect(out.getDate()).toBe(15);
    expect(out.getHours()).toBe(0);
    expect(out.getMinutes()).toBe(30);
    // Resulting interval is +1h.
    expect(out.getTime() - base.getTime()).toBe(60 * 60 * 1000);
  });

  it("does NOT roll when the time exactly equals the base time", () => {
    const base = new Date(2026, 4, 14, 12, 0, 0);
    const out = combineDateAndTimeAllowingNextDay(base, "12:00")!;
    expect(out.getDate()).toBe(14);
  });
});

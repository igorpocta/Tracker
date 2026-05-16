/**
 * Date helpers for range / day arithmetic.
 *
 * Everything operates in the user's local timezone — Tracker is a personal
 * tool and "what day did I log this on" makes most sense in the wall-clock
 * sense of the person looking at the screen.
 *
 * Conventions:
 *   - "unix seconds" is `Math.floor(Date.now()/1000)` style (matches Rust side).
 *   - "day start / end" are the inclusive lower bound (00:00:00) and the
 *     exclusive upper bound (next day's 00:00:00 - 1s).
 */

/** Return a new `Date` with the time zeroed out (00:00:00.000). */
export function startOfDay(date: Date): Date {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  return d;
}

/** Return a new `Date` at 23:59:59.999 of the given date. */
export function endOfDay(date: Date): Date {
  const d = new Date(date);
  d.setHours(23, 59, 59, 999);
  return d;
}

/** Unix-seconds value for the start of `date`. */
export function dayStartUnixS(date: Date): number {
  return Math.floor(startOfDay(date).getTime() / 1000);
}

/** Unix-seconds value for the end of `date` (inclusive of last second). */
export function dayEndUnixS(date: Date): number {
  return Math.floor(endOfDay(date).getTime() / 1000);
}

/** Range tuple `[from_unix_s, to_unix_s]` covering the calendar day of `date`. */
export function dayRangeUnixS(date: Date): [number, number] {
  return [dayStartUnixS(date), dayEndUnixS(date)];
}

/** Unix-seconds value for today's start. */
export function todayStartUnixS(now: Date = new Date()): number {
  return dayStartUnixS(now);
}

/** Unix-seconds value for today's end. */
export function todayEndUnixS(now: Date = new Date()): number {
  return dayEndUnixS(now);
}

/** Add (or subtract, if negative) `n` days to the supplied date. */
export function addDays(date: Date, n: number): Date {
  const d = new Date(date);
  d.setDate(d.getDate() + n);
  return d;
}

/** True iff `a` and `b` fall on the same calendar day in local time. */
export function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/** Compute the Monday of the ISO week containing `date`. */
export function startOfWeek(date: Date): Date {
  const d = startOfDay(date);
  // JS: Sunday = 0; we want Monday-based, so map 0 → 6 (one full week back).
  const dow = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - dow);
  return d;
}

/** Return the seven dates (Mon..Sun) of the ISO week containing `date`. */
export function weekDays(date: Date): Date[] {
  const monday = startOfWeek(date);
  return Array.from({ length: 7 }, (_, i) => addDays(monday, i));
}

/** Return the last `n` days, ending with `date` (inclusive). Newest first. */
export function lastNDays(date: Date, n: number): Date[] {
  return Array.from({ length: n }, (_, i) => addDays(date, -i));
}

/** First day of the month containing `date`. */
export function startOfMonth(date: Date): Date {
  const d = new Date(date.getFullYear(), date.getMonth(), 1);
  d.setHours(0, 0, 0, 0);
  return d;
}

/** Last day of the month containing `date`. */
export function endOfMonth(date: Date): Date {
  const d = new Date(date.getFullYear(), date.getMonth() + 1, 0);
  d.setHours(23, 59, 59, 999);
  return d;
}

/** First day of the previous month relative to `date`. */
export function startOfPreviousMonth(date: Date): Date {
  return startOfMonth(addDays(startOfMonth(date), -1));
}

/** Last day of the previous month relative to `date`. */
export function endOfPreviousMonth(date: Date): Date {
  return endOfMonth(addDays(startOfMonth(date), -1));
}

/** Short weekday + day-of-month label: e.g. "Mon 13.5". */
export function formatShortDayLabel(date: Date): string {
  const weekday = new Intl.DateTimeFormat(undefined, { weekday: "short" })
    .format(date)
    .replace(/\.$/, "");
  const day = date.getDate();
  const month = date.getMonth() + 1;
  return `${weekday} ${day}.${month}`;
}

/** ISO-ish date label: `YYYY-MM-DD`. Used in CSV filenames. */
export function formatIsoDate(date: Date): string {
  const yyyy = date.getFullYear();
  const mm = `${date.getMonth() + 1}`.padStart(2, "0");
  const dd = `${date.getDate()}`.padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

/**
 * Wall-clock time label in `HH:MM` 24-hour format. The clock comes from
 * the local `getHours` / `getMinutes` getters, not from `toISOString()`,
 * so it matches the user's local timezone. Used by every form-level
 * input that needs to display the time portion of a `Date`.
 */
export function formatHHMM(d: Date): string {
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

/**
 * Parse an `HH:MM` (or `H:M`) string into total minutes past midnight.
 * Returns `null` for malformed input or values outside `0-23 : 0-59`.
 * Shared by AddEntryPanel's duration math and TimeLog's inline cells.
 */
export function parseHHMM(s: string): number | null {
  const m = /^(\d{1,2}):(\d{1,2})$/.exec(s);
  if (!m) return null;
  const h = parseInt(m[1], 10);
  const mm = parseInt(m[2], 10);
  if (h < 0 || h > 23 || mm < 0 || mm > 59) return null;
  return h * 60 + mm;
}

/**
 * Combine the date part of `baseDate` with an `HH:MM` time string into
 * a fresh `Date`. The returned `Date` keeps `baseDate`'s year/month/day
 * and replaces hour/minute (seconds zeroed); time is interpreted as
 * wall-clock in the local timezone.
 *
 * Returns `null` if the time string fails to parse — callers should
 * surface that as a form-validation error rather than fall back to a
 * fabricated value.
 */
export function combineDateAndTime(baseDate: Date, hhmm: string): Date | null {
  const m = /^(\d{1,2}):(\d{1,2})$/.exec(hhmm);
  if (!m) return null;
  const h = parseInt(m[1], 10);
  const mm = parseInt(m[2], 10);
  if (h < 0 || h > 23 || mm < 0 || mm > 59) return null;
  return new Date(
    baseDate.getFullYear(),
    baseDate.getMonth(),
    baseDate.getDate(),
    h,
    mm,
    0,
  );
}

/** Long-form display label, e.g. "Tuesday, 13 May 2026". */
export function formatLongDayLabel(date: Date): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  }).format(date);
}

/** Total number of whole days between two dates (b - a). */
export function daysBetween(a: Date, b: Date): number {
  const ms = startOfDay(b).getTime() - startOfDay(a).getTime();
  return Math.round(ms / 86_400_000);
}

/**
 * Read the working-week mask + non-working days for a date range in one
 * batch, then expose a fast local `isWorkingDayLocal(date, …)` helper.
 *
 * Phase 18B — Item 1: the daily-hours chart needs per-column working-day
 * state. Calling `is_working_day` once per cell would be N RPCs; instead we
 * pull the mask + holiday list once and derive locally.
 */
import { useQuery } from "@tanstack/react-query";

import {
  getWorkingWeekMask,
  listNonWorkingDays,
} from "../api/commands";
import type { NonWorkingDay } from "../api/commands";

const DEFAULT_MASK = 0b0011111; // Mon..Fri (Monday = bit 0).

function toIsoDate(d: Date): string {
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

export interface CalendarMaskData {
  /** Working-week bitmask (Monday = bit 0). 0..127. */
  mask: number;
  /** Non-working day rows in the queried range. */
  nonWorking: Set<string>;
}

/**
 * Fetch the working-week mask + non-working days for `[from, to]`. Cached
 * by the date range key so a stable parent re-render doesn't refetch.
 */
export function useCalendarMask(from: Date, to: Date): CalendarMaskData {
  const fromIso = toIsoDate(from);
  const toIso = toIsoDate(to);

  const maskQ = useQuery({
    queryKey: ["working-week-mask"],
    queryFn: getWorkingWeekMask,
    staleTime: 60_000,
  });

  const daysQ = useQuery({
    queryKey: ["non-working-days", fromIso, toIso],
    queryFn: () => listNonWorkingDays(fromIso, toIso),
    staleTime: 60_000,
  });

  const mask = maskQ.data ?? DEFAULT_MASK;
  const nonWorking = new Set<string>(
    (daysQ.data ?? []).map((d: NonWorkingDay) => d.date),
  );

  return { mask, nonWorking };
}

/**
 * Pure helper: is the supplied date a working day given the mask + the
 * non-working-day set? Monday = bit 0; Sunday = bit 6.
 */
export function isWorkingDayLocal(
  date: Date,
  mask: number,
  nonWorking: Set<string>,
): boolean {
  // JS getDay: 0=Sun..6=Sat. Convert to ISO weekday (1=Mon..7=Sun) then to
  // bit index (0=Mon..6=Sun).
  const jsDay = date.getDay();
  const isoIdx = (jsDay + 6) % 7; // 0=Mon..6=Sun
  const inMask = (mask & (1 << isoIdx)) !== 0;
  if (!inMask) return false;
  return !nonWorking.has(toIsoDate(date));
}

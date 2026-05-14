/**
 * `useDateRange` — wraps `useState` for the Reports range picker.
 *
 * Supports five preset ranges and a custom one. Each preset is resolved to
 * a `[from, to]` tuple of `Date`s relative to "now" when first selected;
 * subsequent re-renders return the same pair (the hook owns it).
 */
import { useMemo, useState } from "react";

import {
  addDays,
  endOfMonth,
  endOfPreviousMonth,
  startOfDay,
  startOfMonth,
  startOfPreviousMonth,
} from "../lib/dates";

export type RangePreset =
  | "last_7"
  | "last_30"
  | "this_month"
  | "last_month"
  | "custom";

export interface DateRangeValue {
  preset: RangePreset;
  from: Date;
  to: Date;
}

/** Compute the date range a preset resolves to. */
export function presetRange(preset: RangePreset, now: Date = new Date()): {
  from: Date;
  to: Date;
} {
  const today = startOfDay(now);
  switch (preset) {
    case "last_7":
      return { from: addDays(today, -6), to: today };
    case "last_30":
      return { from: addDays(today, -29), to: today };
    case "this_month":
      return { from: startOfMonth(now), to: endOfMonth(now) };
    case "last_month":
      return { from: startOfPreviousMonth(now), to: endOfPreviousMonth(now) };
    case "custom":
    default:
      return { from: addDays(today, -6), to: today };
  }
}

export interface UseDateRangeReturn extends DateRangeValue {
  setPreset: (next: RangePreset) => void;
  setFrom: (date: Date) => void;
  setTo: (date: Date) => void;
}

export function useDateRange(
  initialPreset: RangePreset = "last_7",
): UseDateRangeReturn {
  const initial = useMemo(() => presetRange(initialPreset), [initialPreset]);
  const [preset, setPresetRaw] = useState<RangePreset>(initialPreset);
  const [from, setFrom] = useState<Date>(initial.from);
  const [to, setTo] = useState<Date>(initial.to);

  const setPreset = (next: RangePreset) => {
    setPresetRaw(next);
    if (next !== "custom") {
      const r = presetRange(next);
      setFrom(r.from);
      setTo(r.to);
    }
  };

  return { preset, from, to, setPreset, setFrom, setTo };
}

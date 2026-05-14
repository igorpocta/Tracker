/**
 * `useTodayBoundary` — yields the current local date and a tick that
 * advances whenever the day rolls over.
 *
 * Phase 18A — Item 9: if the user leaves the app open past midnight, the
 * Today view must re-evaluate so it shows the new day's data, not yesterday's.
 *
 * The hook combines:
 *   1. A 1-minute interval tick (catches drift / forgotten event listeners).
 *   2. The `day-rollover` Tauri event emitted by the backend at local midnight.
 *
 * Returns an object containing:
 *   - `dateLabel`: the current local date in `YYYY-MM-DD`.
 *   - `startUnix`: the unix seconds of today's local 00:00.
 *   - `endUnix`:   the unix seconds of today's local 23:59:59.
 *   - `rolloverCount`: increments each time we detect a date change. Use as
 *     a React-Query key or `useEffect` dep to force refetches.
 */
import { useEffect, useMemo, useRef, useState } from "react";

import { useTauriEvent } from "./useTauriEvent";

export interface TodayBoundary {
  dateLabel: string;
  startUnix: number;
  endUnix: number;
  rolloverCount: number;
}

function todayLabel(now: Date): string {
  const y = now.getFullYear();
  const m = `${now.getMonth() + 1}`.padStart(2, "0");
  const d = `${now.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function localDayBounds(now: Date): { start: number; end: number } {
  const start = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
    0,
    0,
    0,
    0,
  );
  const end = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate(),
    23,
    59,
    59,
    0,
  );
  return {
    start: Math.floor(start.getTime() / 1000),
    end: Math.floor(end.getTime() / 1000),
  };
}

export function useTodayBoundary(): TodayBoundary {
  const [tick, setTick] = useState(0);
  const lastLabel = useRef<string>(todayLabel(new Date()));

  // 1) Minute-resolution interval: catches drift even if the backend event
  //    is missed.
  useEffect(() => {
    const id = window.setInterval(() => {
      const label = todayLabel(new Date());
      if (label !== lastLabel.current) {
        lastLabel.current = label;
        setTick((n) => n + 1);
      }
    }, 60_000);
    return () => window.clearInterval(id);
  }, []);

  // 2) Backend-emitted `day-rollover` event from the local-midnight task.
  useTauriEvent("day-rollover", () => {
    const label = todayLabel(new Date());
    lastLabel.current = label;
    setTick((n) => n + 1);
  });

  return useMemo(() => {
    const now = new Date();
    const { start, end } = localDayBounds(now);
    return {
      dateLabel: todayLabel(now),
      startUnix: start,
      endUnix: end,
      rolloverCount: tick,
    };
    // Recompute whenever the tick changes — that's the whole point of the hook.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tick]);
}

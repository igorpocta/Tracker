/**
 * Tests for `useTodayBoundary` (Phase 18A — Item 9, the day-rollover hook).
 *
 * Coverage: initial day label, minute-tick that catches midnight drift, and
 * the Tauri `day-rollover` event being forwarded into the `rolloverCount`.
 */
import { act, renderHook } from "@testing-library/react";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

import { useTodayBoundary } from "./useTodayBoundary";

// Capture the `day-rollover` listener so we can fire it manually.
let rolloverListener: (() => void) | null = null;
vi.mock("./useTauriEvent", () => ({
  useTauriEvent: (event: string, cb: () => void) => {
    if (event === "day-rollover") rolloverListener = cb;
  },
}));

describe("useTodayBoundary", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    // Anchor the clock on a stable mid-afternoon so day-bounds math is
    // deterministic regardless of when this test runs.
    vi.setSystemTime(new Date(2026, 4, 14, 14, 30, 0, 0));
    rolloverListener = null;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns today's date label + bounds on first render", () => {
    const { result } = renderHook(() => useTodayBoundary());
    expect(result.current.dateLabel).toBe("2026-05-14");
    expect(result.current.rolloverCount).toBe(0);

    // Bounds: local midnight start, 23:59:59 end.
    const start = new Date(2026, 4, 14, 0, 0, 0).getTime() / 1000;
    const end = new Date(2026, 4, 14, 23, 59, 59).getTime() / 1000;
    expect(result.current.startUnix).toBe(start);
    expect(result.current.endUnix).toBe(end);
  });

  it("bumps rolloverCount when the wall clock crosses midnight", () => {
    const { result } = renderHook(() => useTodayBoundary());

    // Advance to the next day; tick the minute interval to detect the change.
    act(() => {
      vi.setSystemTime(new Date(2026, 4, 15, 0, 0, 5, 0));
      vi.advanceTimersByTime(60_000);
    });

    expect(result.current.dateLabel).toBe("2026-05-15");
    expect(result.current.rolloverCount).toBe(1);
  });

  it("responds to the Tauri day-rollover event", () => {
    const { result } = renderHook(() => useTodayBoundary());
    expect(result.current.rolloverCount).toBe(0);
    act(() => {
      vi.setSystemTime(new Date(2026, 4, 15, 0, 0, 1, 0));
      rolloverListener?.();
    });
    expect(result.current.rolloverCount).toBe(1);
    expect(result.current.dateLabel).toBe("2026-05-15");
  });

  it("does NOT bump rolloverCount on intra-day minute ticks", () => {
    const { result } = renderHook(() => useTodayBoundary());
    const initial = result.current.rolloverCount;
    act(() => {
      vi.advanceTimersByTime(60_000 * 3);
    });
    expect(result.current.rolloverCount).toBe(initial);
  });
});

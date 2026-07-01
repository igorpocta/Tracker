import { describe, expect, it, vi } from "vitest";

import type { ActiveTimerState } from "../api/types";
import { toggleTimer } from "./toggleTimer";

const RUNNING: ActiveTimerState = {
  issue_key: "PROJ-1",
  started_at: 0,
  elapsed_seconds: 0,
};

function deps(over: {
  busy?: boolean;
  active?: ActiveTimerState | null;
}) {
  const start = vi.fn(async () => {});
  const stop = vi.fn(async () => null);
  return {
    start,
    stop,
    d: {
      isBusy: () => over.busy ?? false,
      getActive: async () => over.active ?? null,
      start,
      stop,
    },
  };
}

describe("toggleTimer", () => {
  it("starts an unassigned timer when none is running", async () => {
    const { start, stop, d } = deps({ active: null });
    await toggleTimer(d);
    expect(start).toHaveBeenCalledTimes(1);
    expect(stop).not.toHaveBeenCalled();
  });

  it("stops the timer when one is running", async () => {
    const { start, stop, d } = deps({ active: RUNNING });
    await toggleTimer(d);
    expect(stop).toHaveBeenCalledTimes(1);
    expect(start).not.toHaveBeenCalled();
  });

  it("does nothing while a timer command is already in-flight", async () => {
    const { start, stop, d } = deps({ busy: true, active: null });
    await toggleTimer(d);
    expect(start).not.toHaveBeenCalled();
    expect(stop).not.toHaveBeenCalled();
  });
});

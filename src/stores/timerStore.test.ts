/**
 * Vitest coverage for the timer Zustand store. We mock the IPC layer so the
 * store's actions exercise their happy/sad paths without touching Tauri.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api/commands", () => ({
  getTimerState: vi.fn(),
  startTimer: vi.fn(),
  stopTimer: vi.fn(),
  updateTimerStart: vi.fn(),
  updateTimerComment: vi.fn(),
}));

import * as commands from "../api/commands";
import { elapsedSeconds, useTimerStore } from "./timerStore";

const { getTimerState, startTimer, stopTimer, updateTimerStart, updateTimerComment } =
  commands as unknown as {
    getTimerState: ReturnType<typeof vi.fn>;
    startTimer: ReturnType<typeof vi.fn>;
    stopTimer: ReturnType<typeof vi.fn>;
    updateTimerStart: ReturnType<typeof vi.fn>;
    updateTimerComment: ReturnType<typeof vi.fn>;
  };

function resetStore() {
  useTimerStore.setState({ active: null, busy: false, error: null });
}

describe("timerStore", () => {
  beforeEach(() => {
    resetStore();
    getTimerState.mockReset();
    startTimer.mockReset();
    stopTimer.mockReset();
    updateTimerStart.mockReset();
    updateTimerComment.mockReset();
  });

  it("hydrate writes the backend snapshot into the store", async () => {
    getTimerState.mockResolvedValueOnce({
      issue_key: "A-1",
      started_at: 1000,
      elapsed_seconds: 5,
    });
    await useTimerStore.getState().hydrate();
    expect(useTimerStore.getState().active?.issue_key).toBe("A-1");
  });

  it("hydrate tolerates null (no active timer)", async () => {
    getTimerState.mockResolvedValueOnce(null);
    await useTimerStore.getState().hydrate();
    expect(useTimerStore.getState().active).toBeNull();
  });

  it("start sets active on success", async () => {
    startTimer.mockResolvedValueOnce({
      issue_key: "A-1",
      started_at: 2000,
      elapsed_seconds: 0,
    });
    await useTimerStore.getState().start("A-1");
    expect(startTimer).toHaveBeenCalledWith("A-1", undefined, null);
    expect(useTimerStore.getState().active?.issue_key).toBe("A-1");
    expect(useTimerStore.getState().busy).toBe(false);
  });

  it("start records error and rethrows on failure", async () => {
    startTimer.mockRejectedValueOnce("oops");
    await expect(useTimerStore.getState().start("A-1")).rejects.toBe("oops");
    expect(useTimerStore.getState().error).toBe("oops");
    expect(useTimerStore.getState().busy).toBe(false);
  });

  it("stop clears active state and returns the worklog row", async () => {
    useTimerStore.setState({
      active: { issue_key: "A-1", started_at: 0, elapsed_seconds: 30 },
    });
    stopTimer.mockResolvedValueOnce({
      issue_key: "A-1",
      duration_s: 30,
      started_at: 0,
      logged_at: 30,
    });
    const row = await useTimerStore.getState().stop("done");
    expect(stopTimer).toHaveBeenCalledWith("done");
    expect(row?.duration_s).toBe(30);
    expect(useTimerStore.getState().active).toBeNull();
  });

  it("updateStart no-ops when no timer is running", async () => {
    await useTimerStore.getState().updateStart(123);
    expect(updateTimerStart).not.toHaveBeenCalled();
  });

  it("updateStart forwards to the backend when active", async () => {
    useTimerStore.setState({
      active: { issue_key: "A-1", started_at: 0, elapsed_seconds: 10 },
    });
    updateTimerStart.mockResolvedValueOnce({
      issue_key: "A-1",
      started_at: 500,
      elapsed_seconds: 5,
    });
    await useTimerStore.getState().updateStart(500);
    expect(updateTimerStart).toHaveBeenCalledWith(500);
    expect(useTimerStore.getState().active?.started_at).toBe(500);
  });

  it("setComment forwards to the backend when active", async () => {
    useTimerStore.setState({
      active: { issue_key: "A-1", started_at: 0, elapsed_seconds: 10 },
    });
    updateTimerComment.mockResolvedValueOnce({
      issue_key: "A-1",
      started_at: 0,
      elapsed_seconds: 10,
      comment: "hello",
    });
    await useTimerStore.getState().setComment("hello");
    expect(updateTimerComment).toHaveBeenCalledWith("hello");
    expect(useTimerStore.getState().active?.comment).toBe("hello");
  });

  it("setComment no-ops when no timer is running", async () => {
    await useTimerStore.getState().setComment("hello");
    expect(updateTimerComment).not.toHaveBeenCalled();
  });
});

describe("elapsedSeconds helper", () => {
  it("returns 0 when no timer is active", () => {
    expect(elapsedSeconds(null, 5000)).toBe(0);
  });

  it("clamps to 0 for negative diffs", () => {
    expect(
      elapsedSeconds(
        { issue_key: "x", started_at: 10_000, elapsed_seconds: 0 },
        5_000,
      ),
    ).toBe(0);
  });

  it("returns floor((now - started)/1000)", () => {
    expect(
      elapsedSeconds(
        { issue_key: "x", started_at: 1_000, elapsed_seconds: 0 },
        7_500,
      ),
    ).toBe(6);
  });
});

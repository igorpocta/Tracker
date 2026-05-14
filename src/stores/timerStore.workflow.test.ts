/**
 * Workflow / integration tests for the timer Zustand store.
 *
 * Walks the store through a realistic user journey:
 *
 *   1. Mount with no active timer (hydrate → null).
 *   2. Start a timer for ACME-1 with an in-flight comment.
 *   3. Adjust the start time backwards.
 *   4. Edit the comment.
 *   5. Stop with a new comment, observe the worklog row, store cleared.
 *
 * These tests target the seam BETWEEN UI components and the backend (the
 * store + command facade), not a single function. Each step is verified
 * end-to-end including the values passed to the mocked IPC layer.
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
import { useTimerStore } from "./timerStore";

const mocked = commands as unknown as {
  getTimerState: ReturnType<typeof vi.fn>;
  startTimer: ReturnType<typeof vi.fn>;
  stopTimer: ReturnType<typeof vi.fn>;
  updateTimerStart: ReturnType<typeof vi.fn>;
  updateTimerComment: ReturnType<typeof vi.fn>;
};

function reset() {
  useTimerStore.setState({ active: null, busy: false, error: null });
  Object.values(mocked).forEach((m) => m.mockReset());
}

describe("Timer workflow — start → adjust → comment → stop", () => {
  beforeEach(reset);

  it("walks a full session end-to-end", async () => {
    // 1) Hydrate with no active timer.
    mocked.getTimerState.mockResolvedValueOnce(null);
    await useTimerStore.getState().hydrate();
    expect(useTimerStore.getState().active).toBeNull();

    // 2) Start ACME-1 with an in-flight comment.
    mocked.startTimer.mockResolvedValueOnce({
      issue_key: "ACME-1",
      started_at: 1_000_000,
      elapsed_seconds: 0,
      comment: "investigating crash",
    });
    await useTimerStore.getState().start("ACME-1", "investigating crash");
    expect(mocked.startTimer).toHaveBeenCalledWith(
      "ACME-1",
      undefined,
      "investigating crash",
    );
    expect(useTimerStore.getState().active?.issue_key).toBe("ACME-1");
    expect(useTimerStore.getState().active?.comment).toBe(
      "investigating crash",
    );

    // 3) User scrubs the start time back 5 minutes.
    mocked.updateTimerStart.mockResolvedValueOnce({
      issue_key: "ACME-1",
      started_at: 700_000,
      elapsed_seconds: 300,
      comment: "investigating crash",
    });
    await useTimerStore.getState().updateStart(700_000);
    expect(mocked.updateTimerStart).toHaveBeenCalledWith(700_000);
    expect(useTimerStore.getState().active?.started_at).toBe(700_000);

    // 4) User refines the comment.
    mocked.updateTimerComment.mockResolvedValueOnce({
      issue_key: "ACME-1",
      started_at: 700_000,
      elapsed_seconds: 300,
      comment: "fixed null deref in cache::issues",
    });
    await useTimerStore.getState().setComment("fixed null deref in cache::issues");
    expect(useTimerStore.getState().active?.comment).toBe(
      "fixed null deref in cache::issues",
    );

    // 5) Stop with a final dialog comment.
    mocked.stopTimer.mockResolvedValueOnce({
      issue_key: "ACME-1",
      duration_s: 300,
      started_at: 700_000,
      logged_at: 1_000_000,
      comment: "fixed null deref in cache::issues",
    });
    const row = await useTimerStore.getState().stop(
      "fixed null deref in cache::issues",
    );
    expect(mocked.stopTimer).toHaveBeenCalledWith(
      "fixed null deref in cache::issues",
    );
    expect(row?.duration_s).toBe(300);
    expect(row?.comment).toBe("fixed null deref in cache::issues");

    // Store cleared after stop.
    expect(useTimerStore.getState().active).toBeNull();
    expect(useTimerStore.getState().busy).toBe(false);
    expect(useTimerStore.getState().error).toBeNull();
  });

  it("surfaces a Jira-side error without leaving the store in busy state", async () => {
    mocked.startTimer.mockRejectedValueOnce("Jira: 429 too many requests");
    await expect(useTimerStore.getState().start("ACME-1")).rejects.toBe(
      "Jira: 429 too many requests",
    );
    expect(useTimerStore.getState().busy).toBe(false);
    expect(useTimerStore.getState().error).toBe(
      "Jira: 429 too many requests",
    );
    expect(useTimerStore.getState().active).toBeNull();
  });

  it("allows starting again after a failed start", async () => {
    mocked.startTimer.mockRejectedValueOnce("network");
    await expect(useTimerStore.getState().start("X-1")).rejects.toBe(
      "network",
    );
    mocked.startTimer.mockResolvedValueOnce({
      issue_key: "X-1",
      started_at: 5_000,
      elapsed_seconds: 0,
    });
    await useTimerStore.getState().start("X-1");
    expect(useTimerStore.getState().active?.issue_key).toBe("X-1");
    expect(useTimerStore.getState().error).toBeNull();
  });

  it("setComment is a no-op when there is no active timer", async () => {
    await useTimerStore.getState().setComment("late comment");
    expect(mocked.updateTimerComment).not.toHaveBeenCalled();
  });
});

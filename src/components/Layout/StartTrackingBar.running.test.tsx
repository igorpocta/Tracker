/**
 * Running-state coverage for `StartTrackingBar` → `RunningBar`.
 *
 * Regression target: while a timer is running the issue chip was a
 * read-only label, so a wrong/blank task could not be corrected without
 * stopping the timer. The chip now opens an inline issue picker that
 * reassigns the running timer in place (the elapsed clock keeps running).
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api/commands", async (orig) => {
  const actual = await orig<typeof import("../../api/commands")>();
  return {
    ...actual,
    listFavorites: vi.fn().mockResolvedValue([]),
    getSuggestedIssues: vi
      .fn()
      .mockResolvedValue([
        { issue_key: "B-2", summary: "Druhý úkol", updated_at: 0 },
      ]),
    searchIssuesCache: vi
      .fn()
      .mockResolvedValue([
        { issue_key: "B-2", summary: "Druhý úkol", updated_at: 0 },
      ]),
    assignActiveTimer: vi
      .fn()
      .mockResolvedValue({ issue_key: "B-2", started_at: 1000, elapsed_seconds: 5 }),
  };
});

import * as commands from "../../api/commands";
import { useTimerStore } from "../../stores/timerStore";
import { StartTrackingBar } from "./StartTrackingBar";

const assignActiveTimer = commands.assignActiveTimer as unknown as ReturnType<
  typeof vi.fn
>;

function renderBar(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={client}>{node}</QueryClientProvider>);
}

describe("RunningBar — reassign the running timer", () => {
  beforeEach(() => {
    assignActiveTimer.mockClear();
    useTimerStore.setState({
      active: { issue_key: "A-1", started_at: 1000, elapsed_seconds: 5 },
      busy: false,
      error: null,
    });
  });

  afterEach(() => {
    useTimerStore.setState({ active: null, busy: false, error: null });
  });

  it("clicking the issue chip opens an inline issue search", async () => {
    const user = userEvent.setup();
    renderBar(<StartTrackingBar onPickIssue={vi.fn()} onStop={vi.fn()} />);

    // The chip shows the current issue key and is the trigger.
    await user.click(screen.getByText("A-1"));

    expect(
      await screen.findByPlaceholderText("Hledat úkol…"),
    ).toBeInTheDocument();
  });

  it("picking a different issue reassigns the timer in place", async () => {
    const user = userEvent.setup();
    renderBar(
      <StartTrackingBar
        onPickIssue={vi.fn()}
        onStop={vi.fn()}
        onReassign={(k) => useTimerStore.getState().assign(k)}
      />,
    );

    await user.click(screen.getByText("A-1"));
    // Suggested feed surfaces B-2; pick it.
    await user.click(await screen.findByText("Druhý úkol"));

    await waitFor(() =>
      expect(assignActiveTimer).toHaveBeenCalledWith("B-2"),
    );
    // Store now reflects the reassigned issue while the clock keeps running.
    await waitFor(() =>
      expect(useTimerStore.getState().active?.issue_key).toBe("B-2"),
    );
    expect(useTimerStore.getState().active?.started_at).toBe(1000);
  });

  it("delegates reassign to onReassign instead of calling the store directly", async () => {
    // Regression: RunningBar used to call the rethrowing `timerStore.assign`
    // straight from IssuePicker's no-catch handler, so a failed reassign left
    // an unhandled rejection, a stuck-open popover and no toast. Reassign now
    // routes through the `onReassign` prop, where AppShell catches + toasts.
    const onReassign = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderBar(
      <StartTrackingBar
        onPickIssue={vi.fn()}
        onStop={vi.fn()}
        onReassign={onReassign}
      />,
    );

    await user.click(screen.getByText("A-1"));
    await user.click(await screen.findByText("Druhý úkol"));

    await waitFor(() => expect(onReassign).toHaveBeenCalledWith("B-2"));
    // The raw store command must NOT be reached directly from the bar.
    expect(assignActiveTimer).not.toHaveBeenCalled();
  });
});

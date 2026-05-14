/**
 * Smoke + interaction tests for the popover view.
 *
 * Mocks the Tauri IPC bridge so `getRecentIssues`, `getTimerState`, and
 * `startTimer` etc. resolve from fakes instead of actually hitting the
 * backend.
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, eventMock, mockInvoke } from "./test/__mocks__/tauri";
import { Popover } from "./popover";

vi.mock("@tauri-apps/api/core", () => coreMock);
vi.mock("@tauri-apps/api/event", () => eventMock);

function setupInvoke({
  timer = null,
  recent = [],
}: {
  timer?: unknown;
  recent?: unknown[];
}) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_timer_state":
        return timer;
      case "get_recent_issues":
        return recent;
      case "start_timer":
        return {
          issue_key: "ACME-1",
          started_at: Date.now(),
          elapsed_seconds: 0,
        };
      case "stop_timer_inner":
        return null;
      case "open_main_window":
        return null;
      default:
        return null;
    }
  });
}

beforeEach(() => {
  mockInvoke.mockReset();
  eventMock.listen.mockClear();
});

describe("Popover", () => {
  it("renders the idle placeholder when no timer is running", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);

    // Initial render shows the dashes placeholder.
    expect(screen.getByLabelText(/timer not running/i)).toBeInTheDocument();

    // After hydrate the recent-issues empty message appears.
    await waitFor(() =>
      expect(screen.getByText(/no recent issues yet/i)).toBeInTheDocument(),
    );

    // Stop button is disabled when there is no active timer.
    const stopBtn = screen.getByRole("button", { name: /stop timer/i });
    expect(stopBtn).toBeDisabled();
  });

  it("lists recent issues and starts a timer on click", async () => {
    setupInvoke({
      timer: null,
      recent: [
        { issue_key: "ACME-1", summary: "fix the bug", updated_at: 0 },
        { issue_key: "ACME-2", summary: "another thing", updated_at: 0 },
      ],
    });

    render(<Popover />);

    // Both rows show up.
    await waitFor(() => {
      expect(screen.getByText("ACME-1")).toBeInTheDocument();
      expect(screen.getByText("fix the bug")).toBeInTheDocument();
      expect(screen.getByText("ACME-2")).toBeInTheDocument();
    });

    const row = screen
      .getByText("fix the bug")
      .closest("button") as HTMLButtonElement;
    expect(row).toBeTruthy();

    await userEvent.click(row);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "start_timer",
        expect.objectContaining({ issueKey: "ACME-1" }),
      );
    });
  });

  it("invokes open_main_window when 'Open main app' is clicked", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);
    await waitFor(() =>
      expect(screen.getByText(/no recent issues yet/i)).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByRole("button", { name: /open main app/i }));
    expect(mockInvoke).toHaveBeenCalledWith("open_main_window");
  });
});

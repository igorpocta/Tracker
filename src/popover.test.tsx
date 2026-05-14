/**
 * Smoke + interaction tests for the popover view.
 *
 * Mocks the Tauri IPC bridge so `getRecentIssues`, `getTimerState`, etc.
 * resolve from fakes instead of actually hitting the backend.
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
  worklogs = [],
  goal = 9 * 3600,
}: {
  timer?: unknown;
  recent?: unknown[];
  worklogs?: unknown[];
  goal?: number;
}) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_timer_state":
        return timer;
      case "get_recent_issues":
        return recent;
      case "get_worklogs_for_range":
        return worklogs;
      case "get_daily_goal":
        return goal;
      case "get_accent_color":
        return "aurora";
      case "get_theme":
        return "auto";
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
      case "enter_main_app":
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
  it("renders the idle status card when no timer is running", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);

    // After hydrate the idle status card appears.
    await waitFor(() =>
      expect(screen.getByText(/žádná časomíra neběží/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/klikni na úkol pro spuštění/i)).toBeInTheDocument();
  });

  it("renders the Tracker. brand and Dnešní cíl block", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);

    await waitFor(() =>
      expect(screen.getByText(/tracker\./i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/dnešní cíl/i)).toBeInTheDocument();
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

  it("invokes open_main_window when 'Otevřít aplikaci' is clicked", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);
    await waitFor(() =>
      expect(screen.getByText(/zatím žádné nedávné úkoly/i)).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByRole("button", { name: /otevřít aplikaci/i }));
    expect(mockInvoke).toHaveBeenCalledWith("open_main_window");
  });

  it("renders the Nastavení and Ukončit footer buttons", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /nastavení/i })).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: /ukončit/i })).toBeInTheDocument();
  });

  it("subscribes to popover:opened to refetch timer state (Phase 18B Item 17)", async () => {
    setupInvoke({ timer: null, recent: [] });

    render(<Popover />);
    await waitFor(() => {
      // The popover wires up many listeners — `popover:opened` must be one.
      const listenedEvents = (
        eventMock.listen.mock.calls as unknown as Array<[string, unknown]>
      ).map((call) => call[0]);
      expect(listenedEvents).toContain("popover:opened");
      expect(listenedEvents).toContain("timer-started");
      expect(listenedEvents).toContain("timer-stopped");
      expect(listenedEvents).toContain("timer-updated");
      expect(listenedEvents).toContain("worklog-saved");
    });
  });
});

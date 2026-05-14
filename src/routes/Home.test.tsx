/**
 * Smoke / interaction tests for the main app shell.
 *
 * We mock `@tauri-apps/api/core` so the typed wrappers in `src/api/commands`
 * call our spy. The event module is mocked too (no-op listen + emit).
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, eventMock, mockInvoke } from "../test/__mocks__/tauri";
import { usePrefsStore } from "../stores/prefsStore";
import { useTimerStore } from "../stores/timerStore";
import Home from "./Home";

vi.mock("@tauri-apps/api/core", () => coreMock);
vi.mock("@tauri-apps/api/event", () => eventMock);

function renderHome() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Home />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function defaultMocks() {
  mockInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_timer_state":
        return null;
      case "get_daily_goal":
        return 8 * 60 * 60;
      case "get_hourly_rate":
        return 0;
      case "get_recent_issues":
        return [
          {
            issue_key: "ACME-1",
            summary: "fix the bug",
            updated_at: 0,
          },
        ];
      case "get_suggested_issues":
        return [];
      case "get_worklog_issues":
        return [];
      case "search_issues_cache":
        return [];
      case "refresh_cache":
        return 0;
      case "start_timer":
        return {
          issue_key: "ACME-1",
          started_at: Date.now(),
          elapsed_seconds: 0,
        };
      case "stop_timer_inner":
        return {
          issue_key: "ACME-1",
          duration_s: 60,
          started_at: 0,
          logged_at: 0,
        };
      default:
        throw new Error(`unexpected command: ${cmd}`);
    }
  });
}

describe("Home shell", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    eventMock.listen.mockClear();
    // Reset shared Zustand stores so prior tests don't leak state in.
    useTimerStore.setState({ active: null, busy: false, error: null });
    usePrefsStore.setState({
      dailyGoalSeconds: 8 * 60 * 60,
      hourlyRate: 0,
      hydrated: false,
      error: null,
    });
    defaultMocks();
  });

  it("renders the brand + empty timer face", async () => {
    renderHome();
    expect(screen.getByText("Tracker")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByText("--:--:--")).toBeInTheDocument(),
    );
  });

  it("loads recent issues into the sidebar", async () => {
    renderHome();
    await waitFor(() =>
      expect(screen.getByText("ACME-1")).toBeInTheDocument(),
    );
    expect(screen.getByText("fix the bug")).toBeInTheDocument();
  });

  it("subscribes to backend events", async () => {
    renderHome();
    await waitFor(() => {
      // We expect at minimum: worklog-saved, worklog-error, cache-refreshed,
      // prefs-changed, timer-started.
      const names = (eventMock.listen.mock.calls as unknown[][]).map(
        (c) => c[0] as string,
      );
      expect(names).toEqual(
        expect.arrayContaining([
          "worklog-saved",
          "worklog-error",
          "cache-refreshed",
          "prefs-changed",
          "timer-started",
        ]),
      );
    });
  });

  it("clicking an issue and pressing Start invokes the timer command", async () => {
    const user = userEvent.setup();
    renderHome();
    await waitFor(() =>
      expect(screen.getByText("ACME-1")).toBeInTheDocument(),
    );
    await user.click(
      screen.getByRole("button", { name: /ACME-1.*fix the bug/i }),
    );
    const startBtn = await screen.findByRole("button", {
      name: /^Start ACME-1$/i,
    });
    await user.click(startBtn);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("start_timer", {
        issueKey: "ACME-1",
        startedAtMs: null,
      });
    });
  });

  it("clicking the sync pill invokes refresh_cache", async () => {
    const user = userEvent.setup();
    renderHome();
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /refresh issue cache/i }),
      ).toBeInTheDocument();
    });
    await user.click(
      screen.getByRole("button", { name: /refresh issue cache/i }),
    );
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("refresh_cache");
    });
  });

  it("renders the daily goal progress bar with default 8h goal", async () => {
    renderHome();
    await waitFor(() =>
      expect(
        screen.getByRole("progressbar", { name: /daily goal progress/i }),
      ).toBeInTheDocument(),
    );
  });
});

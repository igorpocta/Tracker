/**
 * Smoke tests for the Today route.
 *
 * We mount the route inside a minimal AppShell-equivalent wrapper so that
 * `useOutletContext` returns something usable, and stub all Tauri commands.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockInvoke, coreMock, eventMock } = vi.hoisted(() => {
  const invokeFn = vi.fn();
  return {
    mockInvoke: invokeFn,
    coreMock: { invoke: invokeFn },
    eventMock: {
      listen: vi.fn(async () => () => {}),
      emit: vi.fn(async () => {}),
      emitTo: vi.fn(async () => {}),
    },
  };
});

vi.mock("@tauri-apps/api/core", () => coreMock);
vi.mock("@tauri-apps/api/event", () => eventMock);

import type { ShellOutletContext } from "../components/Layout/AppShell";
import { usePrefsStore } from "../stores/prefsStore";
import { useTimerStore } from "../stores/timerStore";
import Today from "./Today";

const noopCtx: ShellOutletContext = {
  pushToast: () => {},
  openStopDialog: () => {},
};

function renderToday() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Routes>
          <Route element={<Outlet context={noopCtx} />}>
            <Route index element={<Today />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function defaultMocks() {
  mockInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_worklogs_for_range":
        return [];
      case "get_recent_issues":
        return [
          { issue_key: "ACME-1", summary: "fix the bug", updated_at: 0 },
        ];
      case "get_suggested_issues":
        return [];
      case "search_issues_cache":
        return [];
      default:
        return null;
    }
  });
}

describe("Today route", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    eventMock.listen.mockClear();
    useTimerStore.setState({ active: null, busy: false, error: null });
    usePrefsStore.setState({
      dailyGoalSeconds: 8 * 60 * 60,
      hourlyRate: 0,
      hydrated: true,
      error: null,
      currency: "CZK",
      widgetFormat: "HH:MM:SS",
      theme: "dark",
      fontSize: "md",
      density: "comfortable",
    });
    defaultMocks();
  });

  it("renders the empty timer face by default", async () => {
    renderToday();
    expect(await screen.findByText("--:--:--")).toBeInTheDocument();
    expect(screen.getByText(/Pick an issue below to start/i)).toBeInTheDocument();
  });

  it("loads recent issues into the quick-start panel", async () => {
    renderToday();
    await waitFor(() => {
      expect(screen.getByText("ACME-1")).toBeInTheDocument();
      expect(screen.getByText("fix the bug")).toBeInTheDocument();
    });
  });

  it("calls get_worklogs_for_range on mount", async () => {
    renderToday();
    await waitFor(() => {
      const cmds = mockInvoke.mock.calls.map((c) => c[0]);
      expect(cmds).toContain("get_worklogs_for_range");
    });
  });

  it("renders the daily goal progress bar", async () => {
    renderToday();
    await waitFor(() =>
      expect(
        screen.getByRole("progressbar", { name: /daily goal progress/i }),
      ).toBeInTheDocument(),
    );
  });

  it("shows the today empty state when no worklogs", async () => {
    renderToday();
    expect(
      await screen.findByText(/No worklogs yet today/i),
    ).toBeInTheDocument();
  });
});

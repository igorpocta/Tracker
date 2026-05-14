import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
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

import { useTimerStore } from "../stores/timerStore";
import History from "./History";

function renderHistory() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <History />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("History route", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    eventMock.listen.mockClear();
    useTimerStore.setState({ active: null, busy: false, error: null });
    const todayStartS = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_worklogs_for_range") {
        return [
          {
            issue_key: "ACME-7",
            summary: "today task",
            duration_s: 30 * 60,
            started_at: todayStartS + 9 * 3600,
            logged_at: todayStartS + 10 * 3600,
          },
        ];
      }
      return null;
    });
  });

  it("renders the day picker with last 30 days", async () => {
    renderHistory();
    await waitFor(() =>
      expect(screen.getByText("Today")).toBeInTheDocument(),
    );
    expect(screen.getByText("Yesterday")).toBeInTheDocument();
  });

  it("shows the week sparkline header", async () => {
    renderHistory();
    expect(await screen.findByText(/this week/i)).toBeInTheDocument();
  });

  it("renders today's worklog in the right pane", async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText("ACME-7")).toBeInTheDocument();
      expect(screen.getByText("today task")).toBeInTheDocument();
    });
  });

  it("navigation to previous day works", async () => {
    const user = userEvent.setup();
    renderHistory();
    await waitFor(() => screen.getByText("Today"));
    await user.click(screen.getByRole("button", { name: /previous day/i }));
    // Once Prev is clicked, the "Today" jump button appears in the header.
    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: /^today$/i }).length,
      ).toBeGreaterThan(0),
    );
  });
});

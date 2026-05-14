import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
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

import Reports from "./Reports";

function renderReports() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Reports />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Reports route", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    eventMock.listen.mockClear();
    const nowS = Math.floor(Date.now() / 1000);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_worklogs_for_range") {
        return [
          {
            issue_key: "ACME-1",
            summary: "do thing",
            duration_s: 3600,
            started_at: nowS - 86400,
            logged_at: nowS,
          },
          {
            issue_key: "ACME-2",
            summary: "do other",
            duration_s: 1800,
            started_at: nowS - 86400 * 2,
            logged_at: nowS,
          },
          {
            issue_key: "PROJ-1",
            summary: "thing in project",
            duration_s: 7200,
            started_at: nowS - 86400 * 3,
            logged_at: nowS,
          },
        ];
      }
      return null;
    });
  });

  it("renders the range picker and summary cards", async () => {
    renderReports();
    expect(await screen.findByText(/last 7 days/i)).toBeInTheDocument();
    expect(screen.getByText(/^Total$/)).toBeInTheDocument();
  });

  it("renders the bar chart and donut chart", async () => {
    renderReports();
    await waitFor(() => {
      expect(screen.getByTestId("daily-bar-chart")).toBeInTheDocument();
      expect(screen.getByTestId("project-donut-chart")).toBeInTheDocument();
    });
  });

  it("renders top issues table populated from data", async () => {
    renderReports();
    await waitFor(() => {
      expect(screen.getByText("ACME-1")).toBeInTheDocument();
      expect(screen.getByText("PROJ-1")).toBeInTheDocument();
    });
  });

  it("export button is rendered", async () => {
    renderReports();
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /export csv/i }),
      ).toBeInTheDocument(),
    );
  });
});

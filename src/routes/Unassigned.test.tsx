/**
 * Tests for the "Nepřiřazené" review screen.
 *
 * Coverage:
 * - Empty data renders the "vše přiřazeno" reassurance state.
 * - An unassigned row renders its date/duration and an issue picker; picking
 *   an issue calls `assign_worklog_issue` with the row id + chosen key.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { IssueRow, WorklogRow } from "../api/types";
import { coreMock, mockInvoke } from "../test/__mocks__/tauri";

import Unassigned from "./Unassigned";

vi.mock("@tauri-apps/api/core", () => coreMock);

const PUSH_TOAST = vi.fn();
const SHELL_CTX = {
  pushToast: PUSH_TOAST,
  openStopDialog: vi.fn(),
  openAddEntry: vi.fn(),
};

const ISSUE: IssueRow = {
  issue_key: "DEV-9",
  summary: "Nějaký úkol",
  updated_at: 0,
};

function unassignedRow(partial: Partial<WorklogRow> = {}): WorklogRow {
  return {
    id: partial.id ?? 7,
    issue_key: null,
    duration_s: partial.duration_s ?? 1800, // 30m
    started_at:
      partial.started_at ?? Math.floor(new Date(2026, 4, 14, 15, 10).getTime() / 1000),
    logged_at: 0,
    comment: partial.comment ?? null,
    ...partial,
  };
}

function arrange(rows: WorklogRow[]) {
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_unassigned_worklogs":
        return Promise.resolve(rows);
      case "get_suggested_issues":
        return Promise.resolve([ISSUE]);
      case "search_issues_cache":
        return Promise.resolve([ISSUE]);
      case "assign_worklog_issue":
        return Promise.resolve({ ...unassignedRow(), issue_key: "DEV-9" });
      default:
        return Promise.resolve(null);
    }
  });
}

function renderScreen() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/unassigned"]}>
        <Routes>
          <Route element={<Outlet context={SHELL_CTX} />}>
            <Route path="/unassigned" element={<Unassigned />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("<Unassigned />", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    PUSH_TOAST.mockReset();
  });

  it("shows the all-clear state when nothing is unassigned", async () => {
    arrange([]);
    renderScreen();
    expect(await screen.findByText("Vše přiřazeno 🎉")).toBeInTheDocument();
  });

  it("renders an unassigned row with its duration and a picker", async () => {
    arrange([unassignedRow()]);
    renderScreen();
    // Duration surfaces (30m) — both in the row and the header total — and the
    // assign affordance is present.
    expect((await screen.findAllByText("30m")).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Přiřadit úkol")).toBeInTheDocument();
  });

  it("assigns the chosen issue to the row", async () => {
    arrange([unassignedRow({ id: 42 })]);
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByText("Přiřadit úkol"));
    await user.click(await screen.findByText("Nějaký úkol"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("assign_worklog_issue", {
        worklogId: 42,
        issueKey: "DEV-9",
      }),
    );
  });
});

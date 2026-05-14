/**
 * Unit + integration tests for the Historie změn route.
 *
 * Coverage:
 * - `groupByDay` correctly buckets entries into Dnes / Včera / specific dates.
 * - The route renders entries grouped by day with op badges.
 * - Clicking "Obnovit v Jira" prompts a confirmation, then calls
 *   `restoreDeletedWorklog` with the correct audit id.
 * - The action button on entries that have already been restored shows
 *   "Již obnoveno" instead of an active button.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AuditEntry } from "../api/types";
import { coreMock, mockInvoke } from "../test/__mocks__/tauri";

import Audit, { groupByDay } from "./Audit";

vi.mock("@tauri-apps/api/core", () => coreMock);

function makeEntry(partial: Partial<AuditEntry>): AuditEntry {
  return {
    id: partial.id ?? 1,
    occurred_at: partial.occurred_at ?? Math.floor(Date.now() / 1000),
    op: partial.op ?? "create",
    issue_key: partial.issue_key ?? null,
    worklog_id: partial.worklog_id ?? null,
    before_json: partial.before_json ?? null,
    after_json: partial.after_json ?? null,
    success: partial.success ?? true,
    error: partial.error ?? null,
    source_audit_id: partial.source_audit_id ?? null,
  };
}

const PUSH_TOAST = vi.fn();
const SHELL_CTX = {
  pushToast: PUSH_TOAST,
  openStopDialog: vi.fn(),
  openAddEntry: vi.fn(),
};

function renderAudit() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/audit"]}>
        <Routes>
          <Route element={<Outlet context={SHELL_CTX} />}>
            <Route path="/audit" element={<Audit />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("groupByDay", () => {
  it("groups entries into Dnes / Včera / explicit dates", () => {
    const now = new Date("2026-05-14T14:00:00");
    const today = Math.floor(new Date("2026-05-14T10:00:00").getTime() / 1000);
    const yesterday = Math.floor(
      new Date("2026-05-13T20:00:00").getTime() / 1000,
    );
    const old = Math.floor(new Date("2026-05-09T09:00:00").getTime() / 1000);
    const groups = groupByDay(
      [
        makeEntry({ id: 1, occurred_at: today }),
        makeEntry({ id: 2, occurred_at: today - 1000 }),
        makeEntry({ id: 3, occurred_at: yesterday }),
        makeEntry({ id: 4, occurred_at: old }),
      ],
      now,
    );
    expect(groups[0].label).toBe("Dnes");
    expect(groups[0].entries).toHaveLength(2);
    expect(groups[1].label).toBe("Včera");
    expect(groups[1].entries).toHaveLength(1);
    expect(groups[2].label).toMatch(/9\. 5\. 2026/);
  });

  it("returns empty for empty input", () => {
    expect(groupByDay([])).toEqual([]);
  });
});

describe("Audit route", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    PUSH_TOAST.mockReset();
  });

  it("renders audit entries grouped by day with op badges", async () => {
    const todayTs = Math.floor(Date.now() / 1000);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audit_log") {
        return [
          makeEntry({
            id: 10,
            occurred_at: todayTs,
            op: "delete",
            issue_key: "DEV-1",
            worklog_id: "5001",
            before_json: JSON.stringify({
              issue_key: "DEV-1",
              duration_s: 1800,
              started_at: todayTs - 1800,
              logged_at: todayTs - 1800,
              comment: "Hello",
              source: "jira",
            }),
            success: true,
          }),
          makeEntry({
            id: 11,
            occurred_at: todayTs - 60,
            op: "update",
            issue_key: "DEV-2",
            worklog_id: "5002",
            before_json: JSON.stringify({
              issue_key: "DEV-2",
              duration_s: 1800,
              started_at: todayTs - 3600,
              logged_at: todayTs - 3600,
              comment: "Initial",
              source: "jira",
            }),
            after_json: JSON.stringify({
              issue_key: "DEV-2",
              duration_s: 3600,
              started_at: todayTs - 3600,
              logged_at: todayTs - 3600,
              comment: "Updated",
              source: "jira",
            }),
            success: true,
          }),
        ];
      }
      return null;
    });

    renderAudit();

    expect(
      await screen.findByRole("heading", { name: /historie změn/i }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Dnes")).toBeInTheDocument();
    expect(screen.getAllByText(/Smazáno/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Změněno/i).length).toBeGreaterThan(0);
    expect(screen.getByText("DEV-1")).toBeInTheDocument();
    expect(screen.getByText("DEV-2")).toBeInTheDocument();
  });

  it("clicking Obnovit v Jira confirms then calls restoreDeletedWorklog", async () => {
    const todayTs = Math.floor(Date.now() / 1000);
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "get_audit_log") {
        return [
          makeEntry({
            id: 42,
            occurred_at: todayTs,
            op: "delete",
            issue_key: "DEV-1",
            worklog_id: "5001",
            before_json: JSON.stringify({
              issue_key: "DEV-1",
              duration_s: 1800,
              started_at: todayTs - 1800,
              logged_at: todayTs - 1800,
              comment: "Hello",
              source: "jira",
            }),
            success: true,
          }),
        ];
      }
      if (cmd === "restore_deleted_worklog") {
        // Echo the args so the test can assert on them.
        const a = args as { auditId: number };
        return {
          id: 1,
          issue_key: "DEV-1",
          duration_s: 1800,
          started_at: todayTs - 1800,
          logged_at: todayTs,
          jira_worklog_id: `restored-${a.auditId}`,
          source: "jira",
        };
      }
      return null;
    });

    const user = userEvent.setup();
    renderAudit();

    const restoreBtn = await screen.findByRole("button", {
      name: /obnovit v jira/i,
    });
    await user.click(restoreBtn);
    // Confirmation pair appears.
    const confirmBtn = await screen.findByRole("button", { name: "Obnovit" });
    await user.click(confirmBtn);

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(
          ([cmd, args]) =>
            cmd === "restore_deleted_worklog" &&
            (args as { auditId: number }).auditId === 42,
        ),
      ).toBe(true);
    });
  });

  it("shows 'Již obnoveno' when a newer audit row links back to this entry", async () => {
    const todayTs = Math.floor(Date.now() / 1000);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audit_log") {
        return [
          // The successful restore (newer, top of list) — links to id 42.
          makeEntry({
            id: 43,
            occurred_at: todayTs,
            op: "restore",
            issue_key: "DEV-1",
            worklog_id: "9999",
            after_json: JSON.stringify({
              issue_key: "DEV-1",
              duration_s: 1800,
              started_at: todayTs - 1800,
              logged_at: todayTs,
              source: "jira",
            }),
            success: true,
            source_audit_id: 42,
          }),
          // The original delete.
          makeEntry({
            id: 42,
            occurred_at: todayTs - 30,
            op: "delete",
            issue_key: "DEV-1",
            worklog_id: "5001",
            before_json: JSON.stringify({
              issue_key: "DEV-1",
              duration_s: 1800,
              started_at: todayTs - 1800,
              logged_at: todayTs - 1800,
              comment: "Hello",
              source: "jira",
            }),
            success: true,
          }),
        ];
      }
      return null;
    });

    renderAudit();

    expect(await screen.findByText(/Již obnoveno/i)).toBeInTheDocument();
    // The Obnovit button should NOT be present for the original delete.
    expect(
      screen.queryByRole("button", { name: /obnovit v jira/i }),
    ).not.toBeInTheDocument();
  });

  it("failed audit entries show a 'Zkusit znovu' button instead of restore/revert", async () => {
    const todayTs = Math.floor(Date.now() / 1000);
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_audit_log") {
        return [
          makeEntry({
            id: 50,
            occurred_at: todayTs,
            op: "create",
            issue_key: "DEV-1",
            success: false,
            error: "401 Unauthorized",
            after_json: JSON.stringify({
              issue_key: "DEV-1",
              duration_s: 600,
              started_at: todayTs - 600,
              logged_at: todayTs,
              source: "jira",
            }),
          }),
        ];
      }
      return null;
    });

    renderAudit();
    expect(
      await screen.findByRole("button", { name: /zkusit znovu/i }),
    ).toBeInTheDocument();
    // The status indicator says "Selhalo: 401 Unauthorized" — find by combined text.
    expect(screen.getByText(/Selhalo: 401 Unauthorized/i)).toBeInTheDocument();
  });
});

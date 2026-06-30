/**
 * Tests for the shared assign-worklog handler used by both the Time Log and
 * the Nepřiřazené screen. Extracted so the two routes can't drift apart.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorklogRow } from "../api/types";
import { coreMock, mockInvoke } from "../test/__mocks__/tauri";

import { useAssignWorklog } from "./useAssignWorklog";

vi.mock("@tauri-apps/api/core", () => coreMock);

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function row(partial: Partial<WorklogRow> = {}): WorklogRow {
  return {
    id: partial.id ?? 5,
    issue_key: null,
    duration_s: 1800,
    started_at: 0,
    logged_at: 0,
    ...partial,
  };
}

describe("useAssignWorklog", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("assigns the issue and toasts success", async () => {
    mockInvoke.mockResolvedValue({});
    const pushToast = vi.fn();
    const { result } = renderHook(() => useAssignWorklog(pushToast), { wrapper });

    await result.current(row({ id: 7 }), "DEV-1");

    expect(mockInvoke).toHaveBeenCalledWith("assign_worklog_issue", {
      worklogId: 7,
      issueKey: "DEV-1",
    });
    expect(pushToast).toHaveBeenCalledWith(
      "success",
      expect.stringContaining("DEV-1"),
    );
  });

  it("toasts an error when the backend rejects", async () => {
    mockInvoke.mockRejectedValue("boom");
    const pushToast = vi.fn();
    const { result } = renderHook(() => useAssignWorklog(pushToast), { wrapper });

    await result.current(row(), "DEV-2");

    expect(pushToast).toHaveBeenCalledWith("error", "boom");
  });

  it("is a no-op for rows without an id", async () => {
    const pushToast = vi.fn();
    const { result } = renderHook(() => useAssignWorklog(pushToast), { wrapper });

    await result.current(row({ id: null }), "DEV-3");

    expect(mockInvoke).not.toHaveBeenCalled();
    expect(pushToast).not.toHaveBeenCalled();
  });
});

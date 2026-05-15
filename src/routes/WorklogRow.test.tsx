/**
 * Tests for the inline-edit lifecycle of <WorklogRow />.
 *
 * Regression target: the pre-fix component re-synced its `draft*`
 * state from inside a `useMemo` callback whose deps were `[row.*]`.
 * That's a setState during render — Strict Mode runs renders twice
 * (so each row mutation would fire setState twice) and React
 * Compiler treats it as a memoisation invariant violation.
 *
 * Fix: drafts are now lazy-seeded from the current `row` props at
 * the moment the user clicks into a cell (via the `beginEditing*`
 * helpers). The test below pins that semantic — specifically that
 * the second click into the comment cell, after a `row` mutation,
 * picks up the NEW value, not the stale draft kept in component
 * state from the first edit session.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { WorklogRow as ApiWorklogRow } from "../api/types";
import { WorklogRow } from "./TimeLog";

function row(overrides: Partial<ApiWorklogRow>): ApiWorklogRow {
  // Fixed instant: 2026-05-14 10:00 local → 1 hour entry on ACME-1.
  return {
    id: 1,
    issue_key: "ACME-1",
    summary: "Fix the login bug",
    started_at: new Date(2026, 4, 14, 10, 0, 0).getTime() / 1000,
    duration_s: 3600,
    logged_at: 0,
    comment: "alpha",
    jira_worklog_id: "j-1",
    ...overrides,
  };
}

function withProviders(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={client}>{node}</QueryClientProvider>);
}

describe("<WorklogRow /> inline edit lifecycle", () => {
  it("seeds the comment input from the LATEST row when entering edit mode", async () => {
    const user = userEvent.setup();
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    const onDelete = vi.fn();
    const onAssign = vi.fn();

    // Start with comment = "alpha".
    const { rerender } = withProviders(
      <WorklogRow
        row={row({ comment: "alpha" })}
        onUpdate={onUpdate}
        onDelete={onDelete}
        onAssign={onAssign}
      />,
    );

    // Click the comment cell → enter edit mode, input is seeded
    // from row.comment.
    await user.click(screen.getByTitle("Upravit komentář"));
    const firstInput = screen.getByPlaceholderText("Komentář") as HTMLInputElement;
    expect(firstInput.value).toBe("alpha");

    // Escape out of edit mode without saving.
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByPlaceholderText("Komentář")).not.toBeInTheDocument(),
    );

    // The parent silently mutates the row (e.g. background sync) —
    // re-render with comment = "beta".
    rerender(
      <QueryClientProvider client={new QueryClient()}>
        <WorklogRow
          row={row({ comment: "beta" })}
          onUpdate={onUpdate}
          onDelete={onDelete}
          onAssign={onAssign}
        />
      </QueryClientProvider>,
    );

    // Click the comment cell again → input must NOW seed "beta",
    // not the stale "alpha" left in the draft state from the
    // previous edit session. This is the assertion that pins
    // lazy-seed-from-latest-row — pre-fix, the `useMemo` side-effect
    // would have done the re-sync on row change; lazy-seed defers
    // that to the moment of edit-entry instead.
    await user.click(screen.getByTitle("Upravit komentář"));
    const secondInput = screen.getByPlaceholderText("Komentář") as HTMLInputElement;
    expect(secondInput.value).toBe("beta");
  });
});

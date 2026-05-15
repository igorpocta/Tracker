/**
 * Tests for the dropdown that lives inside `StartTrackingBar`.
 *
 * Regression: pre-fix, the parent only mounted `SearchDropdown` when
 * `results.length > 0`, so the loading and empty-state copy were
 * unreachable. After the fix the parent mounts it whenever `open`,
 * and the dropdown picks its own content based on `loading` +
 * `emptyQuery` + `results`. These tests pin the four state buckets.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { IssueRow } from "../../api/types";
import { SearchDropdown } from "./StartTrackingBar";

/** Wraps each render in a fresh QueryClientProvider because the row items
 * contain `<FavoriteStar>` which calls `useQuery`. Without the provider
 * React Query throws at first render. */
function renderInQueryClient(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={client}>{node}</QueryClientProvider>);
}

function row(issueKey: string, summary: string): IssueRow {
  return {
    issue_key: issueKey,
    summary,
    updated_at: 0,
  };
}

const noop = vi.fn();
const baseProps = {
  favoriteKeys: new Set<string>(),
  highlight: 0,
  onPick: noop,
  onHover: noop,
};

describe("<SearchDropdown />", () => {
  it("shows the empty-query loading copy while the recently-tracked feed is fetching", () => {
    renderInQueryClient(
      <SearchDropdown
        {...baseProps}
        results={[]}
        loading={true}
        emptyQuery={true}
      />,
    );
    expect(screen.getByText("Načítám…")).toBeInTheDocument();
  });

  it("shows the search loading copy while a query is in flight", () => {
    renderInQueryClient(
      <SearchDropdown
        {...baseProps}
        results={[]}
        loading={true}
        emptyQuery={false}
      />,
    );
    expect(screen.getByText("Vyhledávání…")).toBeInTheDocument();
  });

  it("nudges the user to type when the dropdown opens with no history", () => {
    // Empty query, no fetch in flight, nothing in cache → previously
    // showed "Žádné odpovídající úkoly." which is confusing when the
    // user hasn't typed anything yet.
    renderInQueryClient(
      <SearchDropdown
        {...baseProps}
        results={[]}
        loading={false}
        emptyQuery={true}
      />,
    );
    expect(
      screen.getByText("Začněte psát pro vyhledání úkolu."),
    ).toBeInTheDocument();
  });

  it("shows the no-matches copy when a query yielded zero results", () => {
    renderInQueryClient(
      <SearchDropdown
        {...baseProps}
        results={[]}
        loading={false}
        emptyQuery={false}
      />,
    );
    expect(screen.getByText("Žádné odpovídající úkoly.")).toBeInTheDocument();
  });

  it("renders the issue rows when results are present", () => {
    renderInQueryClient(
      <SearchDropdown
        {...baseProps}
        results={[row("ACME-1", "Fix the login bug")]}
        loading={false}
        emptyQuery={true}
      />,
    );
    expect(screen.getByText("ACME-1")).toBeInTheDocument();
    expect(screen.getByText("Fix the login bug")).toBeInTheDocument();
    // None of the empty-state strings leak through.
    expect(screen.queryByText("Načítám…")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Začněte psát pro vyhledání úkolu."),
    ).not.toBeInTheDocument();
  });
});

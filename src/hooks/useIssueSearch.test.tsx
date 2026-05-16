/**
 * Tests for the shared issue-search state hook. Three components
 * (StartTrackingBar, IssuePicker, AddEntryPanel) used to maintain
 * parallel copies of debounce + searchIssues + getSuggestedIssues +
 * results-mixing logic; the regression target is "all three converge
 * on this hook and behave identically".
 *
 * We mock the two API commands so the hook's pure orchestration is
 * exercised without spinning up a Tauri runtime.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { IssueRow } from "../api/types";
import { useIssueSearch } from "./useIssueSearch";

vi.mock("../api/commands", () => ({
  searchIssuesCache: vi.fn(),
  getSuggestedIssues: vi.fn(),
}));

import * as commands from "../api/commands";
const searchMock = vi.mocked(commands.searchIssuesCache);
const suggestedMock = vi.mocked(commands.getSuggestedIssues);

afterEach(() => {
  searchMock.mockReset();
  suggestedMock.mockReset();
});

function row(key: string, summary: string): IssueRow {
  return { issue_key: key, summary, updated_at: 0 };
}

function withClient() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useIssueSearch", () => {
  it("returns suggested issues when the query is empty", async () => {
    suggestedMock.mockResolvedValue([row("ACME-1", "first"), row("ACME-2", "second")]);
    searchMock.mockResolvedValue([]);

    const { result } = renderHook(() => useIssueSearch(), {
      wrapper: withClient(),
    });

    await waitFor(() => expect(result.current.results.length).toBe(2));
    expect(result.current.isEmptyQuery).toBe(true);
    expect(suggestedMock).toHaveBeenCalled();
    expect(searchMock).not.toHaveBeenCalled();
  });

  it("flips to search results after the debounce fires", async () => {
    suggestedMock.mockResolvedValue([row("ACME-1", "from suggestions")]);
    searchMock.mockResolvedValue([row("BRAVO-1", "from search")]);

    const { result } = renderHook(
      () => useIssueSearch({ debounceMs: 10 }),
      { wrapper: withClient() },
    );

    act(() => {
      result.current.setQuery("bravo");
    });

    // After debounce + query resolves, search results show up.
    await waitFor(() => {
      expect(result.current.debounced).toBe("bravo");
      expect(result.current.isEmptyQuery).toBe(false);
      expect(result.current.results.map((r) => r.issue_key)).toEqual(["BRAVO-1"]);
    });
    expect(searchMock).toHaveBeenCalledWith("bravo", 12);
  });

  it("respects enabled=false (no commands fire)", async () => {
    suggestedMock.mockResolvedValue([row("ACME-1", "n/a")]);

    const { result } = renderHook(() => useIssueSearch({ enabled: false }), {
      wrapper: withClient(),
    });

    // Give react-query a beat.
    await new Promise((r) => setTimeout(r, 25));
    expect(suggestedMock).not.toHaveBeenCalled();
    expect(searchMock).not.toHaveBeenCalled();
    expect(result.current.results).toEqual([]);
  });

  it("trims whitespace before issuing the search", async () => {
    searchMock.mockResolvedValue([row("CCC-1", "trimmed")]);
    suggestedMock.mockResolvedValue([]);

    const { result } = renderHook(
      () => useIssueSearch({ debounceMs: 5 }),
      { wrapper: withClient() },
    );

    act(() => {
      result.current.setQuery("   ccc   ");
    });
    await waitFor(() => expect(result.current.debounced).toBe("ccc"));
    expect(searchMock).toHaveBeenCalledWith("ccc", 12);
  });
});

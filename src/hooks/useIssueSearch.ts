/**
 * Shared "issue search dropdown" state hook.
 *
 * Three components — `StartTrackingBar`, `IssuePicker`, and
 * `AddEntryPanel` — each re-implemented the same shape: a debounced
 * `query` string, one React Query for the typed search and another for
 * the empty-state "suggested issues" feed, and a combined `results`
 * array picked between them. The presentation layer differs (full
 * dropdown vs. inline popover) so this hook only owns the STATE; each
 * consumer keeps its own UI.
 *
 * Sharing the keys via the centralised `queryKeys` factory means that
 * (a) `refresh` / `cache-refreshed` events fan out to every open
 * picker, and (b) two pickers open at once share a single backend
 * round-trip per debounced term.
 */
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getSuggestedIssues, searchIssuesCache } from "../api/commands";
import { queryKeys } from "../api/queryKeys";
import type { IssueRow } from "../api/types";

export interface UseIssueSearchOptions {
  /**
   * When `false`, both queries are disabled — useful for dropdowns
   * that should only fetch while the popover is open. Defaults to
   * `true` (always-on).
   */
  enabled?: boolean;
  /** Result-list cap. Defaults to 12. */
  limit?: number;
  /** Debounce window in milliseconds before the search query fires. */
  debounceMs?: number;
}

export interface UseIssueSearchResult {
  /** Raw input value — wire to a controlled `<input>`. */
  query: string;
  setQuery: (next: string) => void;
  /** Debounced + trimmed version of `query`. */
  debounced: string;
  /** Combined result list: `searchIssuesCache` when typing, else `getSuggestedIssues`. */
  results: IssueRow[];
  /**
   * True while EITHER feed is fetching its first page AND we don't yet
   * have data to show. Use for the "Načítám…" / "Vyhledávání…" copy.
   */
  isFetching: boolean;
  /** `debounced.length === 0` — helper for empty-state copy + UI branching. */
  isEmptyQuery: boolean;
}

export function useIssueSearch(
  opts: UseIssueSearchOptions = {},
): UseIssueSearchResult {
  const enabled = opts.enabled ?? true;
  const limit = opts.limit ?? 12;
  const debounceMs = opts.debounceMs ?? 150;

  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), debounceMs);
    return () => window.clearTimeout(t);
  }, [query, debounceMs]);

  // Typed search — gated by enabled + non-empty debounced.
  const searchQ = useQuery({
    queryKey: queryKeys.searchIssues.for(debounced, limit),
    queryFn: () => searchIssuesCache(debounced, limit),
    enabled: enabled && debounced.length > 0,
  });

  // Empty-input "recently-tracked" feed — gated by enabled + empty debounced.
  const suggestedQ = useQuery({
    queryKey: queryKeys.suggestedIssues.list(limit),
    queryFn: () => getSuggestedIssues(limit),
    enabled: enabled && debounced.length === 0,
    staleTime: 30_000,
  });

  const isEmptyQuery = debounced.length === 0;
  const results: IssueRow[] = isEmptyQuery
    ? (suggestedQ.data ?? [])
    : (searchQ.data ?? []);

  const isFetching =
    (isEmptyQuery && suggestedQ.isFetching && (suggestedQ.data ?? []).length === 0) ||
    (!isEmptyQuery && searchQ.isFetching && (searchQ.data ?? []).length === 0);

  return { query, setQuery, debounced, results, isFetching, isEmptyQuery };
}

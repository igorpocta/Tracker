/**
 * Centralised React Query key factories.
 *
 * Add a helper here whenever a queryKey is referenced from more than
 * one place — useQuery call site, mutation onSuccess invalidation,
 * background refresh handler, etc. Keeping the literal strings in one
 * file prevents the "key drift" bug where two consumers fetch the
 * same data under different keys and a third caller's
 * `invalidateQueries` hits yet a different key — leaving the UI
 * stale until the staleTime expires.
 *
 * Convention (TkDodds-style hierarchy):
 *
 *   queryKeys.foo.all   →  ["foo"]                       — prefix; use
 *                                                          for broad
 *                                                          invalidations.
 *   queryKeys.foo.list(arg) → ["foo", "list", arg]       — concrete
 *                                                          fetch keys.
 *
 * `all` is a getter that returns a fresh tuple each call so it can
 * also be passed to `invalidateQueries` directly. `list` is a function
 * because it depends on runtime args.
 *
 * Each helper returns a `readonly` tuple so callers spread the result
 * into `useQuery({ queryKey: queryKeys.foo.list(x) })` and TypeScript
 * keeps the tuple shape.
 */
import type { QueryClient } from "@tanstack/react-query";

export const queryKeys = {
  /**
   * Result of `getSuggestedIssues(limit)` — recently-tracked issue
   * list surfaced in the start-tracking bar empty-state dropdown and
   * the worklog issue picker.
   *
   * Pre-refactor (2026-05-15) these two consumers each invented their
   * own key (`recently-tracked-issues` / `picker-recent`) and the
   * shell's invalidation called `["suggested-issues"]` — so neither
   * dropdown ever invalidated. Now both share `.list(limit)` and the
   * shell invalidates `.all()`.
   */
  suggestedIssues: {
    all: () => ["suggested-issues"] as const,
    list: (limit: number) => ["suggested-issues", "list", limit] as const,
  },

  /** Worklog list queries — `worklog-history` and `worklogs-range`. */
  worklogs: {
    all: () => ["worklogs"] as const,
    history: () => ["worklogs", "history"] as const,
    range: (fromUnix: number, toUnix: number) =>
      ["worklogs", "range", fromUnix, toUnix] as const,
    /** Unassigned worklogs (no issue key) — "Nepřiřazené" screen + badge.
     *  Under the `worklogs` prefix so `invalidateWorklogQueries` refreshes it. */
    unassigned: () => ["worklogs", "unassigned"] as const,
  },

  /** "Recently changed" issues (provider-side) shown in the sidebar. */
  recentIssues: {
    all: () => ["recent-issues"] as const,
  },

  /** Cache-wide issue search (text filter). */
  searchIssues: {
    all: () => ["search-issues"] as const,
    for: (term: string, limit: number) =>
      ["search-issues", term, limit] as const,
  },

  /** `cache::*` snapshot for the sidebar badges. */
  cacheStats: {
    all: () => ["cache-stats"] as const,
  },

  /** Configured `connections` table rows + their per-connection extras. */
  connections: {
    all: () => ["connections"] as const,
  },
  connectionStats: {
    all: () => ["connection-stats"] as const,
    for: (connectionId: number) => ["connection-stats", connectionId] as const,
  },
  syncRuns: {
    all: () => ["sync-runs"] as const,
    list: (limit: number) => ["sync-runs", limit] as const,
  },
  syncErrors: {
    all: () => ["sync-errors"] as const,
  },

  /** Settings → working week / non-working day overrides. */
  workingWeekMask: {
    all: () => ["working-week-mask"] as const,
  },
  nonWorkingDays: {
    all: () => ["non-working-days"] as const,
    range: (fromIso: string, toIso: string) =>
      ["non-working-days", fromIso, toIso] as const,
  },

  /** Pomodoro config row read by Settings → Goals. */
  pomodoroConfig: {
    all: () => ["pomodoro-config"] as const,
  },

  /** Favourite-issues set + per-issue toggle state. */
  favorites: {
    all: () => ["favorites"] as const,
    // Keyed by (connectionId, issueKey): the same issue key in two tenants is
    // two independent favorites, so their toggle state must not share a cache
    // entry. `null` covers the uncontrolled / connection-less lookup.
    one: (issueKey: string, connectionId?: number | null) =>
      ["favorite", connectionId ?? null, issueKey] as const,
  },

  /** Daily-goal streak badge (Reports) — derived from worklogs. */
  streaks: {
    all: () => ["streaks"] as const,
  },

  /** "Jako včera?" smart suggestions — derived from past worklogs. */
  smartSuggestions: {
    all: () => ["smart-suggestions"] as const,
  },

  /** JIRA Přehled dashboard — provider-side data refreshed on cache rebuild. */
  jiraDashboard: {
    all: () => ["jira-dashboard"] as const,
  },
} as const;

/**
 * Convenience: invalidate every query whose data depends on the
 * current set of worklogs.
 *
 * Used after worklog mutations (create / update / delete / push /
 * assign) and after every sync — anywhere a single line saying
 * "the worklog list might have changed" used to fan out into a
 * stack of `queryClient.invalidateQueries({ queryKey: ... })`
 * calls that drift out of sync.
 *
 *   worklogs.all          — both `worklog-history` and `worklogs-range`
 *   suggestedIssues.all   — empty-state "recently tracked" dropdowns
 *   recentIssues.all      — sidebar "Recent" list
 *
 * Search results don't depend on worklogs (they read `issues_v2`),
 * so they're NOT invalidated here — that's `invalidateAfterCacheRefresh`.
 */
export function invalidateWorklogQueries(qc: QueryClient): void {
  qc.invalidateQueries({ queryKey: queryKeys.worklogs.all() });
  qc.invalidateQueries({ queryKey: queryKeys.suggestedIssues.all() });
  qc.invalidateQueries({ queryKey: queryKeys.recentIssues.all() });
  // Derived from worklogs — keep the streak badge + smart suggestions fresh.
  qc.invalidateQueries({ queryKey: queryKeys.streaks.all() });
  qc.invalidateQueries({ queryKey: queryKeys.smartSuggestions.all() });
}

/**
 * Invalidate everything that a "cache rebuild" (full sync, reindex,
 * backup restore) potentially changed: worklog lists, the issue
 * cache the search reads from, the sidebar stats badge.
 */
export function invalidateAfterCacheRefresh(qc: QueryClient): void {
  invalidateWorklogQueries(qc);
  qc.invalidateQueries({ queryKey: queryKeys.searchIssues.all() });
  qc.invalidateQueries({ queryKey: queryKeys.cacheStats.all() });
  qc.invalidateQueries({ queryKey: queryKeys.jiraDashboard.all() });
}

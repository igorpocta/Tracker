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
    /** Prefix used for invalidations — matches every `.list(...)` variant. */
    all: () => ["suggested-issues"] as const,
    /** Concrete key for `getSuggestedIssues(limit)`. */
    list: (limit: number) => ["suggested-issues", "list", limit] as const,
  },
} as const;

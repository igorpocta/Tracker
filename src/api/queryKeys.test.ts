/**
 * Factory invariants. The key reason this file exists at all is to
 * prevent the "list key doesn't share the all() prefix" mistake — if
 * that invariant breaks, `invalidateQueries({ queryKey: all() })`
 * stops invalidating `.list(...)` queries and stale-list bugs come
 * back. So we pin the prefix relationship explicitly here.
 */
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

import {
  invalidateAfterCacheRefresh,
  invalidateWorklogQueries,
  queryKeys,
} from "./queryKeys";

describe("queryKeys.suggestedIssues", () => {
  it("list() starts with the all() prefix", () => {
    const prefix = queryKeys.suggestedIssues.all();
    const list = queryKeys.suggestedIssues.list(20);
    // React Query's prefix-match on invalidateQueries works only when
    // the list key starts with every element of the all() prefix in
    // the same order.
    expect(list.slice(0, prefix.length)).toEqual(prefix);
  });

  it("different limits produce distinct keys", () => {
    expect(queryKeys.suggestedIssues.list(10)).not.toEqual(
      queryKeys.suggestedIssues.list(20),
    );
  });

  it("same limit produces equal keys (reference-stable structurally)", () => {
    expect(queryKeys.suggestedIssues.list(15)).toEqual(
      queryKeys.suggestedIssues.list(15),
    );
  });
});

describe("invalidateWorklogQueries", () => {
  it("invalidates worklog list keys + suggested + recent issues", () => {
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");

    invalidateWorklogQueries(qc);

    expect(spy).toHaveBeenCalledTimes(5);
    expect(spy).toHaveBeenCalledWith({ queryKey: queryKeys.worklogs.all() });
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.suggestedIssues.all(),
    });
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.recentIssues.all(),
    });
    // Streak + smart-suggestion data is derived from worklogs, so it must
    // refresh after a worklog mutation too (was orphaned from the fan-out).
    expect(spy).toHaveBeenCalledWith({ queryKey: queryKeys.streaks.all() });
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.smartSuggestions.all(),
    });
  });

  it("does NOT invalidate searchIssues / cacheStats — those are for cache refresh", () => {
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");

    invalidateWorklogQueries(qc);

    for (const call of spy.mock.calls) {
      const [{ queryKey }] = call as [{ queryKey: readonly unknown[] }];
      expect(queryKey[0]).not.toBe("search-issues");
      expect(queryKey[0]).not.toBe("cache-stats");
    }
  });
});

describe("invalidateAfterCacheRefresh", () => {
  it("invalidates worklog set + search + cache stats", () => {
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");

    invalidateAfterCacheRefresh(qc);

    // Superset of invalidateWorklogQueries (5) + searchIssues + cacheStats +
    // jiraDashboard.
    expect(spy).toHaveBeenCalledTimes(8);
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.searchIssues.all(),
    });
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.cacheStats.all(),
    });
    expect(spy).toHaveBeenCalledWith({
      queryKey: queryKeys.jiraDashboard.all(),
    });
  });
});

describe("queryKeys hierarchy invariants", () => {
  it("every list/range/for/history key starts with its all() prefix", () => {
    const pairs: Array<[readonly unknown[], readonly unknown[]]> = [
      [queryKeys.suggestedIssues.all(), queryKeys.suggestedIssues.list(10)],
      [queryKeys.worklogs.all(), queryKeys.worklogs.history()],
      [queryKeys.worklogs.all(), queryKeys.worklogs.range(1, 2)],
      [queryKeys.searchIssues.all(), queryKeys.searchIssues.for("foo", 5)],
      [queryKeys.connectionStats.all(), queryKeys.connectionStats.for(7)],
      [queryKeys.syncRuns.all(), queryKeys.syncRuns.list(50)],
      [
        queryKeys.nonWorkingDays.all(),
        queryKeys.nonWorkingDays.range("2026-01-01", "2026-04-01"),
      ],
    ];
    for (const [prefix, full] of pairs) {
      expect(full.slice(0, prefix.length)).toEqual(prefix);
    }
  });
});

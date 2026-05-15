/**
 * Factory invariants. The key reason this file exists at all is to
 * prevent the "list key doesn't share the all() prefix" mistake — if
 * that invariant breaks, `invalidateQueries({ queryKey: all() })`
 * stops invalidating `.list(...)` queries and stale-list bugs come
 * back. So we pin the prefix relationship explicitly here.
 */
import { describe, expect, it } from "vitest";

import { queryKeys } from "./queryKeys";

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

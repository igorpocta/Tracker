import { describe, expect, it } from "vitest";

import {
  ALLOW_PRIORITY,
  BLOCK_PRIORITY,
  CATCH_ALL_PRIORITY,
  LOOPBACK_PRIORITY,
  buildFocusRules,
  normalizeSitePattern,
  patternToRegex,
} from "./focusRules.js";

const BASE = {
  active: true,
  generation: 1,
  strict_sites: false,
  block: [],
  allow: [],
  blocked_page: "http://127.0.0.1:27420/blocked",
};

/** Does the compiled pattern actually match this URL? */
const matches = (pattern, url) => new RegExp(patternToRegex(pattern)).test(url);

describe("normalizeSitePattern", () => {
  it("strips scheme, wildcard and www so the spellings collapse", () => {
    const expected = { host: "example.com", path: "" };
    expect(normalizeSitePattern("example.com")).toEqual(expected);
    expect(normalizeSitePattern("*.example.com")).toEqual(expected);
    expect(normalizeSitePattern("www.example.com")).toEqual(expected);
    expect(normalizeSitePattern("HTTPS://Example.com/")).toEqual(expected);
  });

  it("keeps a path prefix but drops query and fragment", () => {
    expect(normalizeSitePattern("reddit.com/r/rust?sort=new#x")).toEqual({
      host: "reddit.com",
      path: "/r/rust",
    });
  });

  it("rejects bare words and non-http schemes", () => {
    expect(normalizeSitePattern("reddit")).toBeNull();
    expect(normalizeSitePattern("ftp://example.com")).toBeNull();
    expect(normalizeSitePattern("")).toBeNull();
  });
});

describe("patternToRegex", () => {
  it("matches the domain and its subdomains", () => {
    expect(matches("reddit.com", "https://reddit.com/")).toBe(true);
    expect(matches("reddit.com", "https://old.reddit.com/r/x")).toBe(true);
    expect(matches("reddit.com", "http://reddit.com:8080/x")).toBe(true);
  });

  it("does not match a domain that merely ends with the pattern", () => {
    expect(matches("reddit.com", "https://notreddit.com/")).toBe(false);
    expect(matches("reddit.com", "https://reddit.com.evil.test/")).toBe(false);
  });

  it("does not match the pattern appearing in a query string", () => {
    expect(matches("reddit.com", "https://evil.test/?x=reddit.com")).toBe(false);
  });

  it("treats a pattern path as a prefix", () => {
    expect(matches("reddit.com/r/rust", "https://reddit.com/r/rust/top")).toBe(true);
    expect(matches("reddit.com/r/rust", "https://reddit.com/r/cats")).toBe(false);
  });

  it("anchors both ends so the whole URL is captured for the redirect", () => {
    const regex = patternToRegex("reddit.com");
    expect(regex.startsWith("^")).toBe(true);
    expect(regex.endsWith("$")).toBe(true);
    const url = "https://reddit.com/r/x?sort=new";
    expect(new RegExp(regex).exec(url)[0]).toBe(url);
  });

  it("returns null for an unusable pattern instead of a matching-everything regex", () => {
    expect(patternToRegex("reddit")).toBeNull();
  });
});

describe("buildFocusRules", () => {
  it("produces nothing while no session is running", () => {
    expect(buildFocusRules({ ...BASE, active: false })).toEqual([]);
    expect(buildFocusRules(null)).toEqual([]);
  });

  it("always allows loopback so the block page stays reachable", () => {
    const rules = buildFocusRules({ ...BASE, strict_sites: true });
    const loopback = rules.find((r) => r.priority === LOOPBACK_PRIORITY);
    expect(loopback.action.type).toBe("allow");
    expect(new RegExp(loopback.condition.regexFilter).test(BASE.blocked_page)).toBe(true);
  });

  it("ranks allow above block above the strict catch-all", () => {
    const rules = buildFocusRules({
      ...BASE,
      strict_sites: true,
      block: ["reddit.com"],
      allow: ["atlassian.net"],
    });
    const priorities = rules.map((r) => r.priority);
    expect(priorities).toContain(ALLOW_PRIORITY);
    expect(priorities).toContain(BLOCK_PRIORITY);
    expect(priorities).toContain(CATCH_ALL_PRIORITY);
    expect(ALLOW_PRIORITY).toBeGreaterThan(BLOCK_PRIORITY);
    expect(BLOCK_PRIORITY).toBeGreaterThan(CATCH_ALL_PRIORITY);
  });

  it("omits the catch-all while nothing is allowed through it", () => {
    const rules = buildFocusRules({ ...BASE, strict_sites: true, allow: [] });
    expect(rules.some((r) => r.priority === CATCH_ALL_PRIORITY)).toBe(false);
  });

  it("omits the catch-all when every allow pattern was unusable", () => {
    const rules = buildFocusRules({ ...BASE, strict_sites: true, allow: ["nonsense"] });
    expect(rules.some((r) => r.priority === CATCH_ALL_PRIORITY)).toBe(false);
  });

  it("omits the catch-all outside strict mode", () => {
    const rules = buildFocusRules({ ...BASE, block: ["reddit.com"] });
    expect(rules.some((r) => r.priority === CATCH_ALL_PRIORITY)).toBe(false);
  });

  it("carries the original URL to the block page", () => {
    const [, blockRule] = buildFocusRules({ ...BASE, block: ["reddit.com"] });
    expect(blockRule.action.type).toBe("redirect");
    expect(blockRule.action.redirect.regexSubstitution).toBe(
      "http://127.0.0.1:27420/blocked?u=\\0",
    );
  });

  it("assigns unique ids from the requested start", () => {
    const rules = buildFocusRules(
      { ...BASE, strict_sites: true, block: ["a.com", "b.com"], allow: ["c.com"] },
      1000,
    );
    const ids = rules.map((r) => r.id);
    expect(ids[0]).toBe(1000);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("skips unusable patterns rather than emitting a broken rule", () => {
    const rules = buildFocusRules({ ...BASE, block: ["reddit", "reddit.com"] });
    // loopback + the one valid pattern
    expect(rules).toHaveLength(2);
  });

  it("refuses to build anything without a block page to redirect to", () => {
    expect(buildFocusRules({ ...BASE, blocked_page: "", block: ["reddit.com"] })).toEqual([]);
  });

  it("only redirects top-level navigations", () => {
    const rules = buildFocusRules({ ...BASE, block: ["reddit.com"] });
    for (const rule of rules) {
      expect(rule.condition.resourceTypes).toEqual(["main_frame"]);
    }
  });
});

import { describe, expect, it } from "vitest";

import {
  ALLOW_PRIORITY,
  blockedUrlFor,
  decideUrl,
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
  it("drops the scheme but keeps the host verbatim", () => {
    const expected = { host: "example.com", path: "", wildcard: false };
    expect(normalizeSitePattern("example.com")).toEqual(expected);
    expect(normalizeSitePattern("HTTPS://Example.com/")).toEqual(expected);
    // `www.` names a host of its own, so it must survive.
    expect(normalizeSitePattern("www.example.com")).toEqual({
      host: "www.example.com",
      path: "",
      wildcard: false,
    });
  });

  it("records the wildcard marker instead of discarding it", () => {
    expect(normalizeSitePattern("*.example.com")).toEqual({
      host: "example.com",
      path: "",
      wildcard: true,
    });
  });

  it("keeps a path prefix but drops query and fragment", () => {
    expect(normalizeSitePattern("reddit.com/r/rust?sort=new#x")).toEqual({
      host: "reddit.com",
      path: "/r/rust",
      wildcard: false,
    });
  });

  it("rejects bare words and non-http schemes", () => {
    expect(normalizeSitePattern("reddit")).toBeNull();
    expect(normalizeSitePattern("ftp://example.com")).toBeNull();
    expect(normalizeSitePattern("")).toBeNull();
  });
});

describe("patternToRegex", () => {
  it("matches only the exact host without a wildcard", () => {
    expect(matches("seznam.cz", "https://seznam.cz/")).toBe(true);
    expect(matches("seznam.cz", "http://seznam.cz:8080/x")).toBe(true);
    expect(matches("seznam.cz", "https://www.seznam.cz/")).toBe(false);
    expect(matches("seznam.cz", "https://email.seznam.cz/")).toBe(false);
  });

  it("matches the apex and every subdomain with a wildcard", () => {
    expect(matches("*.seznam.cz", "https://seznam.cz/")).toBe(true);
    expect(matches("*.seznam.cz", "https://www.seznam.cz/")).toBe(true);
    expect(matches("*.seznam.cz", "https://email.seznam.cz/")).toBe(true);
  });

  it("does not match a domain that merely ends with the pattern", () => {
    expect(matches("reddit.com", "https://notreddit.com/")).toBe(false);
    expect(matches("*.reddit.com", "https://notreddit.com/")).toBe(false);
    expect(matches("*.reddit.com", "https://reddit.com.evil.test/")).toBe(false);
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

  it("keeps loopback in step with the desktop's own check", () => {
    const [loopback] = buildFocusRules({ ...BASE, strict_sites: true, allow: ["x.com"] });
    const re = new RegExp(loopback.condition.regexFilter);
    for (const url of [
      "http://127.0.0.1:27420/blocked",
      "http://127.1.2.3/",
      "http://0.0.0.0:8080/",
      "http://localhost:1420/",
      "http://dev.localhost/",
      "http://[::1]:9000/",
    ]) {
      expect(re.test(url), url).toBe(true);
    }
    // Lookalike domains must not inherit the exemption.
    for (const url of ["https://127.foo.com/", "https://127.0.0.1.nip.io/"]) {
      expect(re.test(url), url).toBe(false);
    }
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

describe("decideUrl", () => {
  // The bridge expands patterns before they reach the browser, so a domain
  // arrives as `*.domain`. These are the shapes the extension really sees.
  const active = { ...BASE, block: ["*.qadata.cz"], allow: [] };

  it("blocks a page a loaded tab may already be sitting on", () => {
    expect(decideUrl(active, "https://www.qadata.cz/")).toBe("block");
    expect(decideUrl(active, "https://qadata.cz/x")).toBe("block");
  });

  it("leaves everything else alone", () => {
    expect(decideUrl(active, "https://example.com/")).toBe("allow");
  });

  it("never touches the block page itself", () => {
    expect(decideUrl(active, "http://127.0.0.1:27420/blocked?u=x")).toBe("allow");
  });

  it("ignores tabs that are not http(s)", () => {
    // `chrome://`, `about:` and friends come back from `tabs.query` too.
    expect(decideUrl(active, "chrome://extensions")).toBe("allow");
    expect(decideUrl(active, undefined)).toBe("allow");
  });

  it("does nothing while no session is running", () => {
    expect(decideUrl({ ...active, active: false }, "https://www.qadata.cz/")).toBe("allow");
  });

  it("lets an allow rule win over a block rule", () => {
    const state = { ...active, allow: ["www.qadata.cz"] };
    expect(decideUrl(state, "https://www.qadata.cz/")).toBe("allow");
    expect(decideUrl(state, "https://mail.qadata.cz/")).toBe("block");
  });

  it("blocks everything unlisted in strict mode, but only once armed", () => {
    const armed = { ...BASE, strict_sites: true, allow: ["*.atlassian.net"] };
    expect(decideUrl(armed, "https://news.ycombinator.com/")).toBe("block");
    expect(decideUrl(armed, "https://team.atlassian.net/x")).toBe("allow");

    const unarmed = { ...BASE, strict_sites: true, allow: [] };
    expect(decideUrl(unarmed, "https://news.ycombinator.com/")).toBe("allow");
  });

  it("carries the original URL to the block page, encoded", () => {
    expect(blockedUrlFor(active, "https://x.com/a?b=1&c=2")).toBe(
      "http://127.0.0.1:27420/blocked?u=https%3A%2F%2Fx.com%2Fa%3Fb%3D1%26c%3D2",
    );
  });
});

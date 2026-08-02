/**
 * Focus-mode rule compiler.
 *
 * Turns the ruleset the Tracker bridge publishes at `GET /focus/state` into
 * `declarativeNetRequest` dynamic rules. The browser evaluates those natively,
 * which is why blocking keeps working while the service worker is asleep — the
 * worker only has to be alive to *install* them.
 *
 * Kept free of `chrome.*` so it can be unit-tested (`focusRules.test.js`).
 *
 * ## Priorities
 *
 * | rule                        | priority |
 * |-----------------------------|----------|
 * | loopback allow              | 100      |
 * | allow patterns              | 3        |
 * | block patterns              | 2        |
 * | strict-mode catch-all       | 1        |
 *
 * `allow` sits above `block` so a narrow exception can be carved out of a
 * broad blocking rule, matching how the desktop side resolves the same
 * conflict. The loopback rule sits above everything because the block page
 * itself is served from `127.0.0.1` — blocking that would strand the tab.
 */

export const LOOPBACK_PRIORITY = 100;
export const ALLOW_PRIORITY = 3;
export const BLOCK_PRIORITY = 2;
export const CATCH_ALL_PRIORITY = 1;

/** Only top-level navigations are redirected; subresources are left alone. */
const RESOURCE_TYPES = ["main_frame"];

/** Escape a literal for embedding in a regular expression. */
export function escapeRegex(literal) {
  return literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Split a user-typed pattern into `{ host, path }`, mirroring the desktop
 * normalisation: scheme is optional, a leading `*.` or `www.` is dropped
 * because subdomains match implicitly.
 *
 * Returns `null` for anything that can't be a host (e.g. a bare word).
 */
export function normalizeSitePattern(raw) {
  if (typeof raw !== "string") return null;
  let rest = raw.trim().toLowerCase();
  if (!rest) return null;

  const schemeAt = rest.indexOf("://");
  if (schemeAt !== -1) {
    const scheme = rest.slice(0, schemeAt);
    if (scheme !== "http" && scheme !== "https") return null;
    rest = rest.slice(schemeAt + 3);
  }

  const cut = rest.search(/[/?#]/);
  let host = cut === -1 ? rest : rest.slice(0, cut);
  let path = cut === -1 ? "" : rest.slice(cut);

  host = host.split("@").pop();
  host = host.split(":")[0].replace(/\.+$/, "");
  if (host.startsWith("*.")) host = host.slice(2);
  if (host.startsWith("www.")) host = host.slice(4);
  if (!host || (!host.includes(".") && host !== "localhost")) return null;

  const queryAt = path.search(/[?#]/);
  if (queryAt !== -1) path = path.slice(0, queryAt);
  if (path === "/") path = "";

  return { host, path };
}

/**
 * Regex matching every URL the pattern covers.
 *
 * Anchored at both ends on purpose: the redirect uses `\0` (the whole match)
 * to carry the original address to the block page, so a regex that only
 * matched a prefix would hand over a truncated URL.
 */
export function patternToRegex(pattern) {
  const parsed = normalizeSitePattern(pattern);
  if (!parsed) return null;
  const host = escapeRegex(parsed.host);
  // `(?:[^/?#]*\.)?` covers subdomains without matching `notexample.com`,
  // because the group has to end in a literal dot.
  const authority = `^https?://(?:[^/?#]*\\.)?${host}(?::\\d+)?`;
  return parsed.path
    ? `${authority}${escapeRegex(parsed.path)}.*$`
    : `${authority}(?:[/?#].*)?$`;
}

function redirectAction(blockedPage) {
  return {
    type: "redirect",
    // `\0` is the whole match, i.e. the full original URL. The desktop side
    // reads it back off the raw query string, so it does not need encoding.
    redirect: { regexSubstitution: `${blockedPage}?u=\\0` },
  };
}

function loopbackRule(id) {
  return {
    id,
    priority: LOOPBACK_PRIORITY,
    action: { type: "allow" },
    condition: {
      regexFilter: "^https?://(?:127\\.\\d+\\.\\d+\\.\\d+|localhost|\\[::1\\])(?::\\d+)?(?:[/?#].*)?$",
      resourceTypes: RESOURCE_TYPES,
    },
  };
}

/**
 * Compile the bridge's state into dynamic rules.
 *
 * Returns an empty array when no session is running — that is what makes
 * stopping Focus instantaneous, and what the failsafe path installs when the
 * bridge goes away.
 *
 * @param {object|null} state    payload from `GET /focus/state`
 * @param {number} startId       first rule id to hand out
 */
export function buildFocusRules(state, startId = 1) {
  if (!state || !state.active) return [];
  const blockedPage = state.blocked_page;
  if (typeof blockedPage !== "string" || !blockedPage) return [];

  let nextId = startId;
  const rules = [loopbackRule(nextId++)];

  for (const pattern of state.allow || []) {
    const regexFilter = patternToRegex(pattern);
    if (!regexFilter) continue;
    rules.push({
      id: nextId++,
      priority: ALLOW_PRIORITY,
      action: { type: "allow" },
      condition: { regexFilter, resourceTypes: RESOURCE_TYPES },
    });
  }

  for (const pattern of state.block || []) {
    const regexFilter = patternToRegex(pattern);
    if (!regexFilter) continue;
    rules.push({
      id: nextId++,
      priority: BLOCK_PRIORITY,
      action: redirectAction(blockedPage),
      condition: { regexFilter, resourceTypes: RESOURCE_TYPES },
    });
  }

  if (state.strict_sites) {
    rules.push({
      id: nextId++,
      priority: CATCH_ALL_PRIORITY,
      action: redirectAction(blockedPage),
      condition: { regexFilter: "^https?://.*$", resourceTypes: RESOURCE_TYPES },
    });
  }

  return rules;
}

/**
 * Background service worker — the only place that talks to the Tracker desktop
 * bridge (http://127.0.0.1:27420). Every request carries the per-install bearer
 * token the user pastes into the popup; the desktop server rejects anything
 * without it, so a random web page can't reach the bridge.
 *
 * Message contract (from content script + popup):
 *   { type: "visible-ticket", ticket }          → POST /visible-ticket  (fire & forget)
 *   { type: "status" }                           → GET  /status
 *   { type: "timer-state" }                      → GET  /timer-state
 *   { type: "visible-ticket-get" }               → GET  /visible-ticket
 *   { type: "start", issueKey }                  → POST /start-timer
 *   { type: "stop", comment? }                   → POST /stop-timer
 *   { type: "focus-status" }                     → local focus-sync state
 *
 * It also owns Focus mode's web blocking — see the section at the bottom.
 */

import { buildFocusRules } from "./focusRules.js";

const BASE = "http://127.0.0.1:27420";

/**
 * Re-register content scripts for user-added self-hosted Jira hosts. Dynamic
 * registrations persist across browser restarts, but an extension reload/update
 * can clear them — so we rebuild from stored `customHosts` on startup, but only
 * for hosts whose permission is still granted.
 */
async function reregisterCustomHosts() {
  try {
    const { customHosts } = await chrome.storage.local.get("customHosts");
    if (!Array.isArray(customHosts) || customHosts.length === 0) return;
    const existing = await chrome.scripting.getRegisteredContentScripts();
    const have = new Set(existing.map((s) => s.id));
    const perms = await chrome.permissions.getAll();
    const grantedOrigins = new Set(perms.origins || []);
    const toAdd = [];
    for (const origin of customHosts) {
      const id = `jira-${origin}`;
      const pattern = `${origin}/*`;
      if (have.has(id) || !grantedOrigins.has(pattern)) continue;
      toAdd.push({
        id,
        matches: [pattern],
        js: ["content.js"],
        runAt: "document_idle",
        persistAcrossSessions: true,
      });
    }
    if (toAdd.length) await chrome.scripting.registerContentScripts(toAdd);
  } catch {
    /* best-effort */
  }
}

chrome.runtime.onStartup?.addListener(reregisterCustomHosts);
chrome.runtime.onInstalled?.addListener(reregisterCustomHosts);

async function getToken() {
  const { token } = await chrome.storage.local.get("token");
  return (token || "").trim();
}

/**
 * Default ceiling on a bridge call. Every request must be bounded: the focus
 * long-poll deliberately holds a connection open, and a desktop that stops
 * answering without closing the socket would otherwise leave the `await`
 * pending forever -- taking the sync loop, and with it the failsafe that
 * clears blocking rules, down with it.
 */
const BRIDGE_TIMEOUT_MS = 10_000;

/** `AbortSignal.timeout` is unavailable on older engines; degrade, don't throw. */
function timeoutSignal(ms) {
  return typeof AbortSignal !== "undefined" && AbortSignal.timeout
    ? AbortSignal.timeout(ms)
    : undefined;
}

async function bridge(path, opts = {}) {
  const token = await getToken();
  if (!token) throw new Error("Chybí bridge token — vlož ho v okně rozšíření.");
  const res = await fetch(BASE + path, {
    method: opts.method || "GET",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: opts.body,
    signal: timeoutSignal(opts.timeoutMs ?? BRIDGE_TIMEOUT_MS),
  });
  if (res.status === 401) throw new Error("Neplatný token (401).");
  if (!res.ok) throw new Error(`Bridge vrátil HTTP ${res.status}.`);
  const text = await res.text();
  return text ? JSON.parse(text) : null;
}

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  // Fire-and-forget: the content script pushing the visible ticket.
  if (msg.type === "visible-ticket") {
    bridge("/visible-ticket", {
      method: "POST",
      body: JSON.stringify(msg.ticket),
    }).catch(() => {
      /* Tracker not running / no token — silent, content script retries */
    });
    return false;
  }

  // Request/response calls from the popup.
  (async () => {
    try {
      let data = null;
      switch (msg.type) {
        case "status":
          data = await bridge("/status");
          break;
        case "timer-state":
          data = await bridge("/timer-state");
          break;
        case "visible-ticket-get":
          data = await bridge("/visible-ticket");
          break;
        case "start":
          data = await bridge("/start-timer", {
            method: "POST",
            body: JSON.stringify({ issue_key: msg.issueKey }),
          });
          break;
        case "stop":
          data = await bridge("/stop-timer", {
            method: "POST",
            body: JSON.stringify({ comment: msg.comment ?? null }),
          });
          break;
        case "focus-status":
          // Local state only — the popup uses it to show whether web blocking
          // is armed, without a round-trip to the desktop.
          data = {
            permission: await hasFocusPermission(),
            generation: focusGeneration,
            rules: (await chrome.declarativeNetRequest.getDynamicRules()).length,
          };
          break;
        default:
          throw new Error(`Neznámý příkaz: ${msg.type}`);
      }
      sendResponse({ ok: true, data });
    } catch (e) {
      sendResponse({ ok: false, error: String((e && e.message) || e) });
    }
  })();
  return true; // keep the message channel open for the async response
});

// -----------------------------------------------------------------------------
// Focus mode
// -----------------------------------------------------------------------------
//
// Blocking itself is done by `declarativeNetRequest`, which the browser
// evaluates natively — so a blocked site stays blocked even while this worker
// is asleep. The worker's only job is keeping the installed rules in step with
// the desktop app.
//
// Two mechanisms, because neither is sufficient alone:
//
//   * a long-poll against `/focus/state?wait=…`, which returns the moment the
//     desktop changes anything, so starting Focus takes effect immediately;
//   * a one-minute alarm, which revives the worker (and the long-poll) after
//     the browser has terminated it for being idle.
//
// **Failsafe.** If the bridge is unreachable twice in a row — Tracker quit,
// crashed, or was never started — every rule is removed. Without that, a
// closed Tracker would leave the browser redirecting to a page that no longer
// answers, and the user would have no way back short of disabling the
// extension.

/** Dynamic rule ids start here, clear of anything else we might add later. */
const FOCUS_RULE_ID_START = 1000;
/** Seconds the bridge may hold a long-poll open. */
const FOCUS_POLL_WAIT_SECONDS = 25;
/** Consecutive failures before we tear the rules down. */
const FOCUS_FAILURES_BEFORE_CLEAR = 2;
/** First backoff step; each further failure doubles it. */
const FOCUS_RETRY_BASE_MS = 2000;
/** Failures to absorb in-loop before handing back to the alarm. */
const FOCUS_MAX_RETRIES = 3;
/** Redirect rules need host access to the sites they act on. */
const FOCUS_ORIGINS = ["*://*/*"];

let focusGeneration = null;
let focusFailures = 0;
let focusLoopRunning = false;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function hasFocusPermission() {
  try {
    return await chrome.permissions.contains({ origins: FOCUS_ORIGINS });
  } catch {
    return false;
  }
}

async function clearFocusRules() {
  try {
    const existing = await chrome.declarativeNetRequest.getDynamicRules();
    const removeRuleIds = existing.map((r) => r.id);
    if (removeRuleIds.length) {
      await chrome.declarativeNetRequest.updateDynamicRules({ removeRuleIds });
    }
  } catch {
    /* nothing installed / API unavailable */
  }
}

async function applyFocusState(state) {
  const addRules = buildFocusRules(state, FOCUS_RULE_ID_START);
  const existing = await chrome.declarativeNetRequest.getDynamicRules();
  await chrome.declarativeNetRequest.updateDynamicRules({
    removeRuleIds: existing.map((r) => r.id),
    addRules,
  });
}

/**
 * One poll round.
 *
 * Rules are only rewritten when the generation actually moved, so the common
 * case — a long-poll that times out unchanged — costs nothing.
 *
 * @returns {"ok"|"error"|"no-permission"}
 */
async function focusSyncOnce(waitSeconds) {
  if (!(await hasFocusPermission())) {
    // The user hasn't granted site access yet; make sure no stale rules
    // linger from a previous grant.
    if (focusGeneration !== null) {
      focusGeneration = null;
      await clearFocusRules();
    }
    return "no-permission";
  }

  const params = new URLSearchParams();
  if (waitSeconds) params.set("wait", String(waitSeconds));
  if (focusGeneration !== null) params.set("gen", String(focusGeneration));

  try {
    // The bridge parks the request for `waitSeconds`; allow that plus slack so
    // a healthy long-poll is never mistaken for a hung one.
    const state = await bridge(`/focus/state?${params.toString()}`, {
      timeoutMs: (waitSeconds + 10) * 1000,
    });
    focusFailures = 0;
    if (state && state.generation !== focusGeneration) {
      focusGeneration = state.generation;
      await applyFocusState(state);
    }
    return "ok";
  } catch {
    focusFailures += 1;
    if (focusFailures >= FOCUS_FAILURES_BEFORE_CLEAR && focusGeneration !== null) {
      focusGeneration = null;
      await clearFocusRules();
    }
    return "error";
  }
}

/**
 * Long-poll until the browser kills this worker. Guarded so the alarm can
 * fire freely without stacking loops.
 *
 * The loop gives up rather than retrying forever. A worker that keeps a timer
 * running never becomes idle, so a fixed retry meant Tracker not being
 * installed -- or simply not running -- cost the user battery indefinitely,
 * on a machine that was doing nothing with Focus mode at all. Handing back to
 * the one-minute alarm costs nothing while the worker sleeps, and a bridge
 * that comes back is picked up within that minute.
 *
 * The backoff still absorbs a short blip in-loop, so restarting Tracker
 * reconnects in seconds rather than at the next alarm.
 *
 * `focusFailures` is shared with the rule teardown deliberately: both want
 * "how many consecutive failures", and it survives loop restarts, so a bridge
 * that stays down settles into one attempt per alarm tick.
 */
async function focusLoop() {
  if (focusLoopRunning) return;
  focusLoopRunning = true;
  try {
    for (;;) {
      const outcome = await focusSyncOnce(FOCUS_POLL_WAIT_SECONDS);
      // Without site access there is nothing to poll for; `permissions.onAdded`
      // brings us back.
      if (outcome === "no-permission") return;
      if (outcome === "ok") continue;
      if (focusFailures > FOCUS_MAX_RETRIES) return;
      await sleep(FOCUS_RETRY_BASE_MS * 2 ** (focusFailures - 1));
    }
  } finally {
    focusLoopRunning = false;
  }
}

chrome.alarms?.create("focus-sync", { periodInMinutes: 1 });
chrome.alarms?.onAlarm.addListener((alarm) => {
  if (alarm.name === "focus-sync") void focusLoop();
});

// A browser restart wipes our in-memory generation but NOT the dynamic rules,
// so start from a clean slate rather than trusting rules we can no longer
// explain.
chrome.runtime.onStartup?.addListener(() => {
  focusGeneration = null;
  void clearFocusRules().then(() => focusLoop());
});
chrome.runtime.onInstalled?.addListener(() => {
  focusGeneration = null;
  void clearFocusRules().then(() => focusLoop());
});

// Granting site access from the popup should take effect without a restart.
chrome.permissions?.onAdded?.addListener(() => void focusLoop());
chrome.permissions?.onRemoved?.addListener(() => {
  focusGeneration = null;
  void clearFocusRules();
});

void focusLoop();

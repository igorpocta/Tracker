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
 */

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

/**
 * Content script injected on Jira Cloud pages.
 *
 * Detects which Jira issue the user is currently looking at (from the URL, with
 * a document-title fallback for the summary) and forwards it to the background
 * service worker, which relays it to the Tracker desktop bridge. Jira Cloud is a
 * single-page app, so we poll for URL/title changes rather than relying on a
 * one-time load.
 */

const ISSUE_KEY = /([A-Z][A-Z0-9]+-\d+)/;

function detectIssue() {
  const { pathname, search, href } = location;

  let key = null;
  // /browse/PROJ-123
  const browse = pathname.match(/\/browse\/([A-Z][A-Z0-9]+-\d+)/);
  if (browse) key = browse[1];
  // ?selectedIssue=PROJ-123 (board / backlog side panel)
  if (!key) {
    const sel = new URLSearchParams(search).get("selectedIssue");
    if (sel) {
      const m = sel.match(ISSUE_KEY);
      if (m) key = m[1];
    }
  }
  // /jira/software/.../issues/PROJ-123  (new nav)
  if (!key) {
    const nav = pathname.match(/\/issues?\/([A-Z][A-Z0-9]+-\d+)/);
    if (nav) key = nav[1];
  }
  if (!key) return null;

  // Summary: strip the trailing " - Jira" and a leading "[KEY]" / "KEY:".
  let summary = document.title.replace(/\s*[-–]\s*Jira.*$/i, "").trim();
  summary = summary
    .replace(new RegExp("^\\[?" + key + "\\]?\\s*[:\\-–]?\\s*"), "")
    .trim();

  return { issue_key: key, summary: summary || null, url: href };
}

let lastSignature = "";

function report() {
  const info = detectIssue();
  const signature = info ? `${info.issue_key}|${info.summary || ""}|${info.url}` : "";
  if (signature === lastSignature) return;
  lastSignature = signature;
  if (!info) return; // leaving an issue view: nothing to push
  try {
    chrome.runtime.sendMessage({ type: "visible-ticket", ticket: info });
  } catch {
    /* background asleep / extension reloading — next tick retries */
  }
}

report();
setInterval(report, 2000);
window.addEventListener("popstate", report);

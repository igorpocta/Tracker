/**
 * Popup UI: shows whether the Tracker bridge is reachable, the issue currently
 * detected in the browser, and Start/Stop controls. All bridge traffic goes
 * through the background service worker (see background.js) so the token never
 * lives in the popup's fetch calls.
 */

const $ = (id) => document.getElementById(id);

function send(message) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage(message, (res) =>
      resolve(res || { ok: false, error: "Rozšíření neodpovědělo." }),
    );
  });
}

function setMsg(text, kind) {
  const el = $("msg");
  el.textContent = text || "";
  el.className = "msg" + (kind ? " " + kind : "");
}

let currentKey = null;
let running = false;

async function refresh() {
  // 1) Is the desktop bridge up?
  const status = await send({ type: "status" });
  if (status.ok) {
    $("status").textContent = "připojeno";
    $("status").className = "badge badge-ok";
  } else {
    $("status").textContent = "nedostupné";
    $("status").className = "badge badge-err";
    setMsg(status.error, "err");
  }

  // 2) What issue is visible in the browser (last thing the content script sent)?
  const vt = await send({ type: "visible-ticket-get" });
  if (vt.ok && vt.data && vt.data.issue_key) {
    currentKey = vt.data.issue_key;
    $("ticket-key").textContent = vt.data.issue_key;
    $("ticket-summary").textContent = vt.data.summary || "";
  } else {
    currentKey = null;
    $("ticket-key").textContent = "—";
    $("ticket-summary").textContent = "Otevři Jira úkol v prohlížeči.";
  }

  // 3) Is a timer already running?
  const ts = await send({ type: "timer-state" });
  running = !!(ts.ok && ts.data && ts.data.issue_key !== undefined);
  const activeKey = ts.ok && ts.data ? ts.data.issue_key : null;

  $("start").disabled = !status.ok || !currentKey || running;
  $("stop").disabled = !status.ok || !running;
  if (running) {
    setMsg(`Běží: ${activeKey || "bez úkolu"}`, "ok");
  }
}

$("start").addEventListener("click", async () => {
  if (!currentKey) return;
  const res = await send({ type: "start", issueKey: currentKey });
  setMsg(res.ok ? `Spuštěno: ${currentKey}` : res.error, res.ok ? "ok" : "err");
  refresh();
});

$("stop").addEventListener("click", async () => {
  const res = await send({ type: "stop" });
  setMsg(res.ok ? "Zastaveno a uloženo." : res.error, res.ok ? "ok" : "err");
  refresh();
});

$("save").addEventListener("click", async () => {
  const token = $("token").value.trim();
  await chrome.storage.local.set({ token });
  setMsg("Token uložen.", "ok");
  refresh();
});

// Prefill the token field (masked) if one is stored.
chrome.storage.local.get("token").then(({ token }) => {
  if (token) $("token").value = token;
});

refresh();

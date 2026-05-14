/**
 * Compact popover renderer.
 *
 * The popover lives in its own webview (`popover.html` / label `"popover"`)
 * with `transparent: true` + `decorations: false`. We fill the window
 * edge-to-edge with a single rounded card (12px corners, soft shadow) — no
 * outer padding lets desktop bleed through, no harsh boundary.
 *
 * Layout:
 *   ┌──────────────────────────────────────────┐
 *   │  Tracking · ACME-1                       │
 *   │  01:23:45                                 │
 *   │  ────────────────                         │
 *   │  Recent                                   │
 *   │    ACME-1   fix the bug                   │
 *   │    ACME-2   another thing                 │
 *   │  ────────────────                         │
 *   │  [Open main]    [Stop]                    │
 *   └──────────────────────────────────────────┘
 */
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import {
  getRecentIssues,
  getTimerState,
  openMainWindow,
  startTimer,
  stopTimer,
} from "./api/commands";
import type { ActiveTimerState, IssueRow } from "./api/types";
import { useNow } from "./hooks/useNow";
import { applyAccent } from "./lib/accent";
import { formatDuration } from "./lib/format";
import { elapsedSeconds } from "./stores/timerStore";

import "./index.css";

/** Number of issues we show in the recent-issues list. */
const RECENT_LIMIT = 5;

export function Popover() {
  const [active, setActive] = useState<ActiveTimerState | null>(null);
  const [recent, setRecent] = useState<IssueRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [t, r] = await Promise.all([
        getTimerState(),
        getRecentIssues(RECENT_LIMIT),
      ]);
      setActive(t ?? null);
      setRecent(r ?? []);
      setError(null);
    } catch (e) {
      setError(errMessage(e));
    }
  }, []);

  // Best-effort accent hydration. We try the backend command; if it isn't
  // available (e.g. running standalone test renders) we silently fall back
  // to the default Apple blue baked into the CSS tokens.
  useEffect(() => {
    (async () => {
      try {
        const { getAccentColor } = await import("./api/commands");
        const accent = await getAccentColor();
        applyAccent(accent);
      } catch {
        /* ignore — defaults are fine */
      }
    })();
  }, []);

  // Initial load.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const events = [
      "popover:opened",
      "timer-started",
      "timer-stopped",
      "worklog-saved",
      "prefs-changed",
    ];
    const unlisteners: Array<() => void> = [];
    events.forEach((ev) => {
      listen(ev, () => {
        void refresh();
      })
        .then((u) => unlisteners.push(u))
        .catch(() => {
          /* listening is best-effort outside Tauri (tests). */
        });
    });
    return () => {
      for (const u of unlisteners) u();
    };
  }, [refresh]);

  const startForIssue = useCallback(
    async (issueKey: string) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      try {
        const next = await startTimer(issueKey);
        setActive(next);
      } catch (e) {
        setError(errMessage(e));
      } finally {
        setBusy(false);
      }
    },
    [busy],
  );

  const stop = useCallback(async () => {
    if (busy || !active) return;
    setBusy(true);
    setError(null);
    try {
      await stopTimer();
      setActive(null);
    } catch (e) {
      setError(errMessage(e));
    } finally {
      setBusy(false);
    }
  }, [busy, active]);

  const openMain = useCallback(async () => {
    try {
      await openMainWindow();
    } catch (e) {
      setError(errMessage(e));
    }
  }, []);

  return (
    <div
      className="h-full w-full flex flex-col overflow-hidden bg-[var(--bg-surface)] text-[var(--text-primary)]"
      style={{
        borderRadius: 12,
        boxShadow: "var(--shadow-popover)",
        // Subtle hairline to crisp the edge in light mode where the shadow alone
        // doesn't carry enough contrast against the desktop wallpaper.
        outline: "0.5px solid var(--border-default)",
        outlineOffset: "-0.5px",
      }}
    >
      <PopoverTimer active={active} />

      <div className="px-3 flex-1 min-h-0 flex flex-col">
        <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-1.5">
          Recent
        </div>
        <div className="flex-1 overflow-y-auto -mr-1 pr-1">
          {recent.length === 0 ? (
            <div className="text-xs text-[var(--text-tertiary)] py-2">
              No recent issues yet.
            </div>
          ) : (
            <ul className="flex flex-col gap-0.5">
              {recent.map((iss) => (
                <li key={iss.issue_key}>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => startForIssue(iss.issue_key)}
                    className="w-full text-left rounded-[var(--radius-sm)] px-2 py-1.5
                               hover:bg-[var(--bg-hover)]
                               disabled:opacity-50 disabled:cursor-not-allowed
                               transition-colors duration-150"
                  >
                    <div className="font-mono text-[10px] uppercase text-[var(--text-secondary)]">
                      {iss.issue_key}
                    </div>
                    <div className="text-xs text-[var(--text-primary)] truncate">
                      {iss.summary || "(no summary)"}
                    </div>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        {error && (
          <div className="text-[11px] text-[var(--danger)] py-1" role="alert">
            {error}
          </div>
        )}
      </div>

      <PopoverActions
        hasActive={active !== null}
        busy={busy}
        onOpenMain={openMain}
        onStop={stop}
      />
    </div>
  );
}

interface PopoverTimerProps {
  active: ActiveTimerState | null;
}

function PopoverTimer({ active }: PopoverTimerProps) {
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);

  return (
    <div className="px-3 pt-3 pb-2">
      <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
        {active ? "Tracking" : "Tracker"}
      </div>
      <div
        className={
          active
            ? "font-mono tabular-nums text-3xl text-[var(--accent)] leading-tight font-light"
            : "font-mono tabular-nums text-3xl text-[var(--text-disabled)] leading-tight font-light"
        }
        aria-live="polite"
        aria-label={
          active ? `Elapsed time ${formatDuration(elapsed)}` : "Timer not running"
        }
      >
        {active ? formatDuration(elapsed) : "--:--:--"}
      </div>
      <div className="font-mono text-[10px] uppercase text-[var(--text-secondary)] mt-1 truncate">
        {active ? active.issue_key : "Idle"}
      </div>
    </div>
  );
}

interface PopoverActionsProps {
  hasActive: boolean;
  busy: boolean;
  onOpenMain: () => void;
  onStop: () => void;
}

function PopoverActions({
  hasActive,
  busy,
  onOpenMain,
  onStop,
}: PopoverActionsProps) {
  return (
    <div className="px-3 py-2.5 border-t border-[var(--border-subtle)] flex items-center gap-2">
      <button
        type="button"
        onClick={onOpenMain}
        className="flex-1 h-7 rounded-[var(--radius-md)] border border-[var(--border-default)]
                   hover:bg-[var(--bg-hover)] text-[var(--text-primary)] text-xs
                   transition-colors duration-150"
      >
        Open main app
      </button>
      <button
        type="button"
        onClick={onStop}
        disabled={!hasActive || busy}
        className="flex-1 h-7 rounded-[var(--radius-md)] bg-[var(--danger)] hover:brightness-110
                   text-white text-xs disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)]
                   disabled:hover:brightness-100 disabled:cursor-not-allowed transition-colors duration-150"
      >
        Stop timer
      </button>
    </div>
  );
}

function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "unknown error";
}

// Only mount when actually loaded in the browser/webview.
const rootEl = document.getElementById("root");
if (rootEl) {
  ReactDOM.createRoot(rootEl).render(<Popover />);
}

/**
 * Compact popover renderer.
 *
 * The popover lives in its own webview (`popover.html` / label `"popover"`)
 * with `transparent: true` + `decorations: false`, so we wrap our content in a
 * rounded dark card with a soft shadow. The window itself is otherwise
 * see-through.
 *
 * Layout:
 *   ┌──────────────────────────────────────────┐
 *   │ Tracker                                  │
 *   │ 01:23:45                                 │
 *   │ ACME-1 · fix the bug                     │
 *   │ ────────────────                         │
 *   │ Recent                                   │
 *   │   [ACME-1] fix the bug                   │
 *   │   [ACME-2] another thing                 │
 *   │   …                                      │
 *   │ ────────────────                         │
 *   │ [Open main app]  [Stop timer]            │
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

  // Initial load.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Re-fetch when:
  //   - the backend reports the popover opened (so it always shows fresh data)
  //   - a worklog is saved (timer just stopped from somewhere else)
  //   - a timer is started/stopped from another surface
  useEffect(() => {
    const events = [
      "popover:opened",
      "timer-started",
      "timer-stopped",
      "worklog-saved",
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
    <div className="h-full w-full p-2">
      <div className="h-full w-full rounded-xl bg-neutral-900/95 shadow-2xl ring-1 ring-white/5 flex flex-col overflow-hidden">
        <PopoverTimer active={active} />

        <div className="px-3 pb-2 flex-1 min-h-0 flex flex-col">
          <div className="text-[11px] uppercase tracking-wider text-neutral-500 mb-1">
            Recent
          </div>
          <div className="flex-1 overflow-y-auto -mr-1 pr-1">
            {recent.length === 0 ? (
              <div className="text-xs text-neutral-500 py-2">
                No recent issues yet.
              </div>
            ) : (
              <ul className="flex flex-col gap-1">
                {recent.map((iss) => (
                  <li key={iss.issue_key}>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => startForIssue(iss.issue_key)}
                      className="w-full text-left rounded-md px-2 py-1.5 hover:bg-neutral-800 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <div className="font-mono text-[11px] text-neutral-400">
                        {iss.issue_key}
                      </div>
                      <div className="text-xs text-neutral-100 truncate">
                        {iss.summary || "(no summary)"}
                      </div>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {error && (
            <div className="text-[11px] text-red-400 py-1" role="alert">
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
      <div className="text-[10px] uppercase tracking-wider text-neutral-500">
        Tracker
      </div>
      <div
        className={
          active
            ? "font-mono tabular-nums text-3xl text-white leading-tight"
            : "font-mono tabular-nums text-3xl text-neutral-600 leading-tight"
        }
        aria-live="polite"
        aria-label={
          active ? `Elapsed time ${formatDuration(elapsed)}` : "Timer not running"
        }
      >
        {active ? formatDuration(elapsed) : "--:--:--"}
      </div>
      <div className="text-xs text-neutral-400 mt-0.5 truncate">
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
    <div className="px-3 py-2 border-t border-white/5 flex items-center gap-2">
      <button
        type="button"
        onClick={onOpenMain}
        className="flex-1 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-100 text-xs py-1.5"
      >
        Open main app
      </button>
      <button
        type="button"
        onClick={onStop}
        disabled={!hasActive || busy}
        className="flex-1 rounded-md bg-red-600 hover:bg-red-500 text-white text-xs py-1.5 disabled:bg-neutral-800 disabled:text-neutral-500 disabled:cursor-not-allowed"
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

/**
 * Popover (menu-bar dropdown) renderer.
 *
 * Reference: `screens/SCR-20260514-rkec-2.png`.
 *
 *   ┌──────────────────────────────────────────────┐
 *   │ Tracker.               Today goal  5h 21m/9h │  ← cursive accent script
 *   │                        ▰▰▰▰▰▰▰░░░░░░░░░░░░░ │
 *   ├──────────────────────────────────────────────┤
 *   │  ⏰  No timer running                         │
 *   │      Click an issue to start tracking         │
 *   ├──────────────────────────────────────────────┤
 *   │  RECENT                                       │
 *   │  [DEV-792]  Portál – Synchronizace…           │
 *   │  [DEV-304]  Úpravy ZZJ v OKO                  │
 *   │  [DEV-926]  Portal – (servis)…                │
 *   ├──────────────────────────────────────────────┤
 *   │  ↗ Open app    ⚙ Settings    ⏻ Quit          │
 *   └──────────────────────────────────────────────┘
 *
 * Auto-hide on focus loss is handled by the Rust side
 * (`src-tauri/src/popover.rs::setup`) — no JS blur handler required.
 */
import { emitTo } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import { ExternalLink, LogOut, Settings as SettingsIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import ReactDOM from "react-dom/client";

import {
  getAccentColor,
  getDailyGoal,
  getRecentIssues,
  getTheme,
  getTimerState,
  getWorklogsForRange,
  openMainWindow,
  startTimer,
} from "./api/commands";
import type { ActiveTimerState, IssueRow, ThemePref, WorklogRow } from "./api/types";
import { useNow } from "./hooks/useNow";
import { applyPalette } from "./lib/accent";
import { todayEndUnixS, todayStartUnixS } from "./lib/dates";
import { formatDuration } from "./lib/format";
import { elapsedSeconds } from "./stores/timerStore";

import "./index.css";

function applyThemeAttr(theme: ThemePref): void {
  if (typeof document === "undefined") return;
  const html = document.documentElement;
  if (theme === "auto") {
    html.removeAttribute("data-theme");
  } else {
    html.setAttribute("data-theme", theme);
  }
}

/**
 * Hydrate theme + palette (mono/dual collapses into one accent set) from the
 * backend so the popover matches whatever the main window looks like. Runs on
 * mount and on every `prefs-changed` event so live changes propagate.
 */
async function hydratePopoverAppearance(): Promise<void> {
  try {
    const [theme, accent] = await Promise.all([
      getTheme().catch<ThemePref>(() => "auto"),
      getAccentColor().catch(() => "aurora"),
    ]);
    applyThemeAttr(theme);
    applyPalette(accent);
  } catch {
    /* defaults are fine */
  }
}

const RECENT_LIMIT = 5;

export function Popover() {
  const [active, setActive] = useState<ActiveTimerState | null>(null);
  const [recent, setRecent] = useState<IssueRow[]>([]);
  const [todayRows, setTodayRows] = useState<WorklogRow[]>([]);
  const [dailyGoalSeconds, setDailyGoalSeconds] = useState<number>(9 * 3600);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const range = [todayStartUnixS(), todayEndUnixS()] as const;
      const [t, r, w, g] = await Promise.all([
        getTimerState(),
        getRecentIssues(RECENT_LIMIT),
        getWorklogsForRange(range[0], range[1]),
        getDailyGoal().catch(() => 9 * 3600),
      ]);
      setActive(t ?? null);
      setRecent(r ?? []);
      setTodayRows(w ?? []);
      setDailyGoalSeconds(g ?? 9 * 3600);
      setError(null);
    } catch (e) {
      setError(errMessage(e));
    }
  }, []);

  // Best-effort theme + palette hydration. Done on mount and again whenever
  // the backend tells us prefs changed (so live theme switches in the main
  // window propagate into an already-open popover).
  useEffect(() => {
    void hydratePopoverAppearance();
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    // Phase 18B — Item 17: keep the popover face in lockstep with the main
    // window. Every relevant event triggers a state refetch so the running
    // timer / today totals / recent issues stay consistent.
    const events = [
      "popover:opened",
      "timer-started",
      "timer-stopped",
      "timer-updated",
      "worklog-saved",
      "worklog-created",
      "worklog-updated",
      "worklog-deleted",
      "worklog-moved",
    ];
    const unlisteners: Array<() => void> = [];
    events.forEach((ev) => {
      listen(ev, () => {
        void refresh();
      })
        .then((u) => unlisteners.push(u))
        .catch(() => {});
    });
    // `prefs-changed` triggers BOTH a data refresh AND an appearance reload.
    listen("prefs-changed", () => {
      void refresh();
      void hydratePopoverAppearance();
    })
      .then((u) => unlisteners.push(u))
      .catch(() => {});
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

  const openMain = useCallback(async () => {
    try {
      await openMainWindow();
    } catch (e) {
      setError(errMessage(e));
    }
  }, []);

  const openSettings = useCallback(async () => {
    try {
      // Show + focus the main window first so the navigate event is acted on.
      await openMainWindow();
      // Then emit the navigate event with payload "settings" directly to the
      // main window. NavigationBridge in App.tsx listens and routes there.
      await emitTo("main", "main-window:navigate", "settings");
    } catch {
      /* ignore */
    }
  }, []);

  const quit = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      // Tauri exposes `exit` via the process plugin; fall back to closing the
      // window if it isn't available.
      await invoke<void>("plugin:process|exit", { code: 0 }).catch(() => {});
    } catch {
      /* ignore */
    }
  }, []);

  return (
    <div
      className="h-full w-full flex flex-col overflow-hidden"
      style={{
        background: "var(--bg-surface)",
        color: "var(--text-primary)",
        borderRadius: 12,
        boxShadow: "var(--shadow-popover)",
        outline: "0.5px solid var(--border-default)",
        outlineOffset: "-0.5px",
      }}
    >
      <Header todayRows={todayRows} dailyGoalSeconds={dailyGoalSeconds} active={active} />

      <StatusCard active={active} />

      <RecentList recent={recent} busy={busy} onPick={startForIssue} />

      {error && (
        <div className="px-4 py-1 text-[11px]" style={{ color: "var(--danger)" }} role="alert">
          {error}
        </div>
      )}

      <Footer onOpen={openMain} onSettings={openSettings} onQuit={quit} />
    </div>
  );
}

// -----------------------------------------------------------------------------

function Header({
  todayRows,
  dailyGoalSeconds,
  active,
}: {
  todayRows: WorklogRow[];
  dailyGoalSeconds: number;
  active: ActiveTimerState | null;
}) {
  const now = useNow(active ? 1000 : 60_000);
  const baseSeconds = useMemo(
    () => todayRows.reduce((a, r) => a + r.duration_s, 0),
    [todayRows],
  );
  const liveSeconds = active ? elapsedSeconds(active, now) : 0;
  const loggedSeconds = baseSeconds + liveSeconds;

  const pct = Math.min(100, (loggedSeconds / Math.max(1, dailyGoalSeconds)) * 100);

  return (
    <div className="px-4 pt-4 pb-3">
      <div className="flex items-end justify-between gap-3">
        <span
          className="text-[26px] leading-none italic font-semibold"
          style={{
            color: "var(--accent)",
            fontFamily: "var(--font-script), serif",
          }}
        >
          Tracker.
        </span>
        <div className="text-right">
          <div className="text-[10px] uppercase tracking-[0.1em] text-[var(--text-tertiary)]">
            Dnešní cíl
          </div>
          <div className="text-sm font-medium tabular-nums" style={{ color: "var(--accent)" }}>
            {shortDuration(loggedSeconds)} / {shortDuration(dailyGoalSeconds)}
          </div>
          <div
            className="h-[3px] mt-1 rounded-full overflow-hidden"
            style={{ background: "var(--bg-active)", width: "140px" }}
          >
            <div
              style={{
                width: `${pct.toFixed(0)}%`,
                height: "100%",
                background: "var(--accent)",
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function StatusCard({ active }: { active: ActiveTimerState | null }) {
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = active ? elapsedSeconds(active, now) : 0;
  return (
    <div className="mx-4 mt-1 mb-3 p-3 rounded-[var(--radius-md)]
                    border border-[var(--border-subtle)] bg-[var(--bg-app)]">
      <div className="min-w-0">
        {active ? (
          <>
            <div className="text-base font-semibold tabular-nums" style={{ color: "var(--accent)" }}>
              {formatDuration(elapsed)}
            </div>
            <div className="text-[11px] text-[var(--text-tertiary)] truncate mt-0.5">
              Sledování {active.issue_key}
            </div>
          </>
        ) : (
          <>
            <div className="text-base font-semibold tabular-nums text-[var(--text-tertiary)]">
              💤 —:—
            </div>
            <div className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
              Klikni na úkol pro spuštění
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function RecentList({
  recent,
  busy,
  onPick,
}: {
  recent: IssueRow[];
  busy: boolean;
  onPick: (key: string) => void;
}) {
  return (
    <div className="px-4 flex-1 min-h-0 flex flex-col">
      <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-2">
        Naposledy
      </div>
      <div className="flex-1 overflow-y-auto -mr-1 pr-1">
        {recent.length === 0 ? (
          <div className="text-xs text-[var(--text-tertiary)] py-2">
            Zatím žádné nedávné úkoly.
          </div>
        ) : (
          <ul className="flex flex-col gap-1">
            {recent.map((iss) => (
              <li key={iss.issue_key}>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onPick(iss.issue_key)}
                  className="w-full text-left rounded-[var(--radius-sm)] px-2 py-1.5
                             hover:bg-[var(--bg-hover)]
                             disabled:opacity-50 disabled:cursor-not-allowed
                             transition-colors duration-150 flex items-center gap-2"
                >
                  <span
                    className="inline-flex items-center justify-center px-2 h-5 rounded-full
                               font-mono text-[10px] uppercase tracking-[0.06em]"
                    style={{
                      color: "var(--accent)",
                      border: "1px solid var(--accent)",
                    }}
                  >
                    {iss.issue_key}
                  </span>
                  <span className="text-xs text-[var(--text-primary)] truncate flex-1">
                    {iss.summary || "(načítá se…)"}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function Footer({
  onOpen,
  onSettings,
  onQuit,
}: {
  onOpen: () => void;
  onSettings: () => void;
  onQuit: () => void;
}) {
  return (
    <div className="grid grid-cols-3 gap-2 px-4 py-3 border-t border-[var(--border-subtle)]">
      <FooterButton icon={<ExternalLink className="w-3.5 h-3.5" />} label="Otevřít aplikaci" onClick={onOpen} />
      <FooterButton icon={<SettingsIcon className="w-3.5 h-3.5" />} label="Nastavení" onClick={onSettings} />
      <FooterButton icon={<LogOut className="w-3.5 h-3.5" />} label="Ukončit" onClick={onQuit} />
    </div>
  );
}

function FooterButton({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center justify-center gap-1.5 h-7 rounded-[var(--radius-sm)]
                 text-[11px] text-[var(--text-secondary)]
                 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]
                 transition-colors duration-150"
    >
      {icon}
      {label}
    </button>
  );
}

function shortDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h === 0) return `${m}m`;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
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

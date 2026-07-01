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
import { Clock, ExternalLink, LogOut, Settings as SettingsIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import ReactDOM from "react-dom/client";

import {
  getAccentColor,
  getDailyGoal,
  getRecentIssues,
  getSuggestedIssues,
  getTheme,
  getTimerState,
  getWorklogsForRange,
  openMainWindow,
  quitApp,
  startTimer,
} from "./api/commands";
import type { ActiveTimerState, IssueRow, ThemePref, WorklogRow } from "./api/types";
import { useT } from "./i18n";
import { useNow } from "./hooks/useNow";
import { useTauriEvent } from "./hooks/useTauriEvent";
import { applyPalette } from "./lib/accent";
import { dayOverlapSeconds, todayEndUnixS, todayStartUnixS } from "./lib/dates";
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
      // "Naposledy trackováno" — issues seřazené podle posledního lokálního
      // worklogu (`getSuggestedIssues`), shoda s hlavním oknem. Pokud uživatel
      // ještě nemá žádné worklogy v cache, padáme zpět na "naposledy upraveno
      // v provideru" (`getRecentIssues`), ať dropdown nebyl prázdný.
      const [t, suggested, w, g] = await Promise.all([
        getTimerState(),
        getSuggestedIssues(RECENT_LIMIT),
        getWorklogsForRange(range[0], range[1]),
        getDailyGoal().catch(() => 9 * 3600),
      ]);
      let recentList = suggested ?? [];
      if (recentList.length === 0) {
        recentList = (await getRecentIssues(RECENT_LIMIT)) ?? [];
      }
      setActive(t ?? null);
      setRecent(recentList);
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

  // Phase 18B — Item 17: keep the popover face in lockstep with the main
  // window. Every relevant event triggers a state refetch so the running
  // timer / today totals / recent issues stay consistent.
  //
  // Pre-fix the listeners were registered with a `listen(...).then(push)`
  // pattern that leaked when the promise resolved AFTER unmount — the
  // resolved unlisten ended up in an array nobody read on cleanup, and
  // Tauri kept the handler alive. `useTauriEvent` carries the canonical
  // `cancelled` guard so the late-resolved unlisten fires immediately.
  const onRefresh = useCallback(() => {
    void refresh();
  }, [refresh]);
  const onPrefsChanged = useCallback(() => {
    void refresh();
    void hydratePopoverAppearance();
  }, [refresh]);
  useTauriEvent("popover:opened", onRefresh);
  useTauriEvent("timer-started", onRefresh);
  useTauriEvent("timer-stopped", onRefresh);
  useTauriEvent("timer-updated", onRefresh);
  useTauriEvent("worklog-saved", onRefresh);
  useTauriEvent("worklog-created", onRefresh);
  useTauriEvent("worklog-updated", onRefresh);
  useTauriEvent("worklog-deleted", onRefresh);
  useTauriEvent("worklog-moved", onRefresh);
  useTauriEvent("prefs-changed", onPrefsChanged);

  const startForIssue = useCallback(
    async (issueKey: string, connectionId?: number | null) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      try {
        // Carry the tenant so a key shared across connections starts the right
        // issue (feedback #4).
        const next = await startTimer(issueKey, undefined, null, connectionId);
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
      await quitApp();
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
  const t = useT();
  const now = useNow(active ? 1000 : 60_000);
  // Variant B (feedback #2): clip each worklog — and the running timer — to
  // today's window, so a cross-midnight entry counts only its in-day slice
  // toward the daily goal. `[todayStart, todayEnd)` half-open (next midnight).
  const todayStart = todayStartUnixS();
  const todayEnd = todayEndUnixS() + 1;
  const baseSeconds = useMemo(
    () =>
      todayRows.reduce(
        (a, r) =>
          a +
          dayOverlapSeconds(
            r.started_at,
            r.ended_at ?? r.started_at + r.duration_s,
            todayStart,
            todayEnd,
          ),
        0,
      ),
    [todayRows, todayStart, todayEnd],
  );
  const liveSeconds = active
    ? dayOverlapSeconds(
        Math.floor(active.started_at / 1000),
        Math.floor(now / 1000),
        todayStart,
        todayEnd,
      )
    : 0;
  const loggedSeconds = baseSeconds + liveSeconds;

  const pct = Math.min(100, (loggedSeconds / Math.max(1, dailyGoalSeconds)) * 100);

  return (
    <div className="px-4 pt-4 pb-3">
      <div className="flex items-end justify-between gap-3">
        <span
          className="text-[22px] leading-none font-semibold tracking-tight"
          style={{ color: "var(--accent)" }}
        >
          Tracker
        </span>
        <div className="text-right">
          <div className="text-[10px] uppercase tracking-[0.1em] text-[var(--text-tertiary)]">
            {t("setup.popover.dailyGoal")}
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
  const t = useT();
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = active ? elapsedSeconds(active, now) : 0;
  return (
    <div className="mx-4 mt-1 mb-3 p-3 rounded-[var(--radius-md)]
                    border border-[var(--border-subtle)] bg-[var(--bg-app)]
                    flex items-center gap-3">
      <span
        aria-hidden
        className="w-9 h-9 rounded-full flex items-center justify-center shrink-0"
        style={{
          background: active ? "var(--accent-soft)" : "var(--bg-active)",
          color: active ? "var(--accent)" : "var(--text-tertiary)",
        }}
      >
        <Clock className="w-4 h-4" />
      </span>
      <div className="min-w-0">
        {active ? (
          <>
            <div className="text-sm font-medium tabular-nums" style={{ color: "var(--accent)" }}>
              {formatDuration(elapsed)}
            </div>
            <div className="text-[11px] text-[var(--text-tertiary)] truncate"
                 title={active.summary ?? undefined}>
              {active.issue_key}
              {active.summary && active.summary.trim().length > 0
                ? ` · ${active.summary}`
                : ""}
            </div>
          </>
        ) : (
          <>
            <div className="text-sm font-medium text-[var(--text-primary)]">
              {t("setup.popover.noTimer")}
            </div>
            <div className="text-[11px] text-[var(--text-tertiary)]">
              {t("setup.popover.clickToStart")}
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
  onPick: (key: string, connectionId?: number | null) => void;
}) {
  const t = useT();
  return (
    <div className="px-4 flex-1 min-h-0 flex flex-col">
      <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-2">
        {t("setup.popover.recent")}
      </div>
      <div className="flex-1 overflow-y-auto -mr-1 pr-1">
        {recent.length === 0 ? (
          <div className="text-xs text-[var(--text-tertiary)] py-2">
            {t("setup.popover.noRecent")}
          </div>
        ) : (
          <ul className="flex flex-col gap-1">
            {recent.map((iss) => (
              <li key={`${iss.connection_id ?? ""} ${iss.issue_key}`}>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onPick(iss.issue_key, iss.connection_id)}
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
                    {iss.summary || t("setup.popover.loadingIssue")}
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
  const t = useT();
  return (
    <div className="grid grid-cols-3 gap-2 px-4 py-3 border-t border-[var(--border-subtle)]">
      <FooterButton icon={<ExternalLink className="w-3.5 h-3.5" />} label={t("setup.popover.openApp")} onClick={onOpen} />
      <FooterButton icon={<SettingsIcon className="w-3.5 h-3.5" />} label={t("setup.popover.settings")} onClick={onSettings} />
      <FooterButton icon={<LogOut className="w-3.5 h-3.5" />} label={t("setup.popover.quit")} onClick={onQuit} />
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
  // Production-only: F12 / Cmd+Opt+I / kontextové menu blok. V dev no-op.
  void import("./lib/devtoolsGuard").then((m) => m.installDevtoolsGuard());
  ReactDOM.createRoot(rootEl).render(<Popover />);
}

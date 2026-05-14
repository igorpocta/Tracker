/**
 * Main tracking app shell.
 *
 * Layout:
 *   ┌────────────────────────── Header ──────────────────────────┐
 *   │ Tracker  [timer chip]            Sync status   ⚙           │
 *   ├──────────┬────────────────────────────────────────────────┤
 *   │ Sidebar  │ Right panel                                     │
 *   │ (issues) │ - Daily goal bar (+ settings)                   │
 *   │          │ - Timer face (when active or selection chosen)  │
 *   │          │ - Issue detail OR worklog history (tabs)        │
 *   └──────────┴────────────────────────────────────────────────┘
 *
 * Drives:
 *   - Backend cache via TanStack Query (recent, suggested, search, worklogs).
 *   - Active timer + prefs via Zustand.
 *   - Tauri events: timer-started, worklog-saved, worklog-error,
 *     cache-refreshed, prefs-changed.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { History, ListTree, Pencil, Square } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  getWorklogIssues,
  refreshCache,
  searchIssuesCache,
} from "../api/commands";
import type {
  ActiveTimerState,
  IssueRow,
  WorklogRow,
} from "../api/types";
import { Button } from "../components/common/Button";
import { IconButton } from "../components/common/IconButton";
import { Spinner } from "../components/common/Spinner";
import {
  Toaster,
  type Toast,
  type ToastVariant,
} from "../components/common/Toast";
import { DailyGoalBar } from "../components/Goal/DailyGoalBar";
import { GoalSettings } from "../components/Goal/GoalSettings";
import { Header } from "../components/Layout/Header";
import { type SyncState } from "../components/Layout/SyncStatus";
import { Sidebar } from "../components/Layout/Sidebar";
import { IssueDetail } from "../components/Issues/IssueDetail";
import { Timer } from "../components/Timer/Timer";
import { StopDialog } from "../components/Timer/TimerControls";
import { WorklogHistory } from "../components/History/WorklogHistory";
import { useNow } from "../hooks/useNow";
import { useTauriEvent } from "../hooks/useTauriEvent";
import { isToday } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";
import { elapsedSeconds, useTimerStore } from "../stores/timerStore";

type RightTab = "detail" | "history";

export default function Home() {
  const queryClient = useQueryClient();

  // ---- timer / prefs hydration ---------------------------------------------
  const hydrateTimer = useTimerStore((s) => s.hydrate);
  const hydratePrefs = usePrefsStore((s) => s.hydrate);
  const startTimer = useTimerStore((s) => s.start);
  const stopTimer = useTimerStore((s) => s.stop);
  const timerBusy = useTimerStore((s) => s.busy);
  const active = useTimerStore((s) => s.active);
  const setActive = useTimerStore((s) => s.setActive);
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);

  useEffect(() => {
    void hydrateTimer();
    void hydratePrefs();
  }, [hydrateTimer, hydratePrefs]);

  // ---- selected issue + right tab ------------------------------------------
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [tab, setTab] = useState<RightTab>("detail");
  const [goalSettingsOpen, setGoalSettingsOpen] = useState(false);
  const [stopOpen, setStopOpen] = useState(false);

  // When a new timer starts elsewhere (tray, deep-link) keep selection in sync.
  useEffect(() => {
    if (active && !selectedKey) {
      setSelectedKey(active.issue_key);
    }
  }, [active, selectedKey]);

  // ---- sync status ----------------------------------------------------------
  const [syncState, setSyncState] = useState<SyncState>({
    kind: "idle",
    lastSyncMs: null,
  });
  const refresh = useCallback(async () => {
    setSyncState({ kind: "syncing" });
    try {
      await refreshCache();
      setSyncState({ kind: "idle", lastSyncMs: Date.now() });
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      queryClient.invalidateQueries({ queryKey: ["search-issues"] });
    } catch (e) {
      const message = typeof e === "string" ? e : (e as Error).message ?? "Refresh failed";
      setSyncState({ kind: "error", message });
    }
  }, [queryClient]);

  // ---- toast notifications --------------------------------------------------
  const toastIdRef = useRef(1);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const pushToast = useCallback((variant: ToastVariant, message: string) => {
    const id = toastIdRef.current++;
    setToasts((prev) => [...prev, { id, variant, message }]);
  }, []);
  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  // ---- selected issue data --------------------------------------------------
  // We don't have a `get_issue_by_key` command, but search() with the key as
  // a substring + exact-match filter is fine for the cache view.
  const selectedIssueQuery = useQuery({
    queryKey: ["issue-by-key", selectedKey],
    queryFn: async () => {
      if (!selectedKey) return null;
      const rows = await searchIssuesCache(selectedKey, 10);
      return rows.find((r) => r.issue_key === selectedKey) ?? null;
    },
    enabled: !!selectedKey,
  });

  // ---- worklog data for the goal bar ---------------------------------------
  const historyQuery = useQuery({
    queryKey: ["worklog-history", 50],
    queryFn: () => getWorklogIssues(50),
  });

  // ---- backend events -------------------------------------------------------
  const onWorklogSaved = useCallback(
    (row: WorklogRow) => {
      pushToast(
        "success",
        `Saved ${formatRow(row)} on ${row.issue_key}.`,
      );
      queryClient.invalidateQueries({ queryKey: ["worklog-history"] });
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
      setStopOpen(false);
    },
    [pushToast, queryClient],
  );
  useTauriEvent<WorklogRow>("worklog-saved", onWorklogSaved);

  const onWorklogError = useCallback(
    (err: unknown) => {
      const msg = typeof err === "string" ? err : "Jira sync failed";
      pushToast("error", `Worklog: ${msg}`);
    },
    [pushToast],
  );
  useTauriEvent<unknown>("worklog-error", onWorklogError);

  const onCacheRefreshed = useCallback(
    (_count: unknown) => {
      setSyncState({ kind: "idle", lastSyncMs: Date.now() });
      queryClient.invalidateQueries({ queryKey: ["recent-issues"] });
      queryClient.invalidateQueries({ queryKey: ["suggested-issues"] });
    },
    [queryClient],
  );
  useTauriEvent<number>("cache-refreshed", onCacheRefreshed);

  const onPrefsChanged = useCallback(() => {
    void hydratePrefs();
  }, [hydratePrefs]);
  useTauriEvent<string>("prefs-changed", onPrefsChanged);

  const onTimerStarted = useCallback(
    (snap: ActiveTimerState) => {
      setActive(snap);
      setSelectedKey(snap.issue_key);
    },
    [setActive],
  );
  useTauriEvent<ActiveTimerState>("timer-started", onTimerStarted);

  // ---- derived totals -------------------------------------------------------
  const now = useNow(active ? 1000 : 60_000);
  const todayLoggedSeconds = useMemo(() => {
    const rows = historyQuery.data ?? [];
    const todayRows = rows.filter((r) => isToday(r.started_at));
    const fromHistory = todayRows.reduce((acc, r) => acc + r.duration_s, 0);
    const fromActive =
      active && isToday(Math.floor(active.started_at / 1000))
        ? elapsedSeconds(active, now)
        : 0;
    return fromHistory + fromActive;
  }, [historyQuery.data, active, now]);

  // ---- handlers -------------------------------------------------------------
  const handleStart = useCallback(
    async (issueKey: string) => {
      try {
        await startTimer(issueKey);
        setSelectedKey(issueKey);
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Failed to start timer");
      }
    },
    [startTimer, pushToast],
  );

  const handleStopConfirm = useCallback(
    async ({
      comment,
      startedAtMs,
    }: {
      comment: string;
      startedAtMs: number | null;
    }) => {
      try {
        if (startedAtMs !== null) {
          await useTimerStore.getState().updateStart(startedAtMs);
        }
        await stopTimer(comment.length > 0 ? comment : undefined);
        // Toast is fired via the `worklog-saved` event handler so we get
        // consistent messages whether the stop originated here or elsewhere.
      } catch (e) {
        pushToast("error", typeof e === "string" ? e : "Failed to save worklog");
      }
    },
    [stopTimer, pushToast],
  );

  // ---- render ---------------------------------------------------------------
  return (
    <div className="h-screen flex flex-col bg-[#0f0f0f] text-neutral-100">
      <Header
        syncState={syncState}
        onRefresh={refresh}
        onOpenSettings={() => setGoalSettingsOpen((v) => !v)}
        onStop={active ? () => setStopOpen(true) : undefined}
      />

      <div className="flex-1 flex min-h-0">
        <Sidebar
          selectedKey={selectedKey}
          activeKey={active?.issue_key ?? null}
          onSelect={(key) => {
            setSelectedKey(key);
            setTab("detail");
          }}
        />

        <main className="flex-1 min-w-0 overflow-y-auto p-6 flex flex-col gap-6">
          {/* Goal bar + settings popover. */}
          <section className="flex flex-col gap-2 max-w-md">
            <div className="flex items-center justify-between gap-2">
              <DailyGoalBar
                loggedSeconds={todayLoggedSeconds}
                goalSeconds={dailyGoalSeconds}
                className="flex-1"
              />
              <IconButton
                aria-label="Edit daily goal"
                onClick={() => setGoalSettingsOpen((v) => !v)}
              >
                <Pencil className="w-3.5 h-3.5" aria-hidden />
              </IconButton>
            </div>
            {hourlyRate > 0 && (
              <div className="text-xs text-neutral-400">
                Today's value:{" "}
                <span className="text-neutral-100 font-medium">
                  {formatMoney((todayLoggedSeconds / 3600) * hourlyRate)}
                </span>{" "}
                ({hourlyRate}/h)
              </div>
            )}
            <GoalSettings
              open={goalSettingsOpen}
              onClose={() => setGoalSettingsOpen(false)}
            />
          </section>

          {/* Timer face. */}
          <section className="flex items-center justify-between gap-6 flex-wrap">
            <div className="flex flex-col gap-1">
              <span className="text-[10px] uppercase tracking-wide text-neutral-500">
                {active ? "Tracking" : "Idle"}
              </span>
              <Timer className="text-5xl" />
              {active && (
                <span className="font-mono text-xs text-neutral-400 mt-1">
                  {active.issue_key}
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              {active ? (
                <Button
                  variant="danger"
                  onClick={() => setStopOpen(true)}
                  disabled={timerBusy}
                >
                  {timerBusy ? (
                    <Spinner className="w-3.5 h-3.5" />
                  ) : (
                    <Square className="w-3.5 h-3.5" aria-hidden />
                  )}
                  Stop & save
                </Button>
              ) : selectedKey ? (
                <Button
                  variant="primary"
                  onClick={() => handleStart(selectedKey)}
                  disabled={timerBusy}
                >
                  Start {selectedKey}
                </Button>
              ) : (
                <span className="text-xs text-neutral-500">
                  Pick an issue to start tracking.
                </span>
              )}
            </div>
          </section>

          {/* Tabs (Detail | History). */}
          <div className="flex items-center gap-2 border-b border-neutral-800/70 -mx-6 px-6">
            <TabButton
              active={tab === "detail"}
              onClick={() => setTab("detail")}
              icon={<ListTree className="w-3.5 h-3.5" aria-hidden />}
            >
              Issue
            </TabButton>
            <TabButton
              active={tab === "history"}
              onClick={() => setTab("history")}
              icon={<History className="w-3.5 h-3.5" aria-hidden />}
            >
              History
            </TabButton>
          </div>

          {tab === "detail" ? (
            <DetailTab
              selectedKey={selectedKey}
              loading={selectedIssueQuery.isLoading}
              issue={selectedIssueQuery.data}
              active={!!active && active.issue_key === selectedKey}
              onStart={handleStart}
              onStop={() => setStopOpen(true)}
            />
          ) : (
            <WorklogHistory />
          )}
        </main>
      </div>

      {active && (
        <StopDialog
          open={stopOpen}
          active={active}
          busy={timerBusy}
          onClose={() => setStopOpen(false)}
          onConfirm={handleStopConfirm}
        />
      )}

      <Toaster toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  children,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={`inline-flex items-center gap-1.5 px-2.5 py-2 text-xs transition-colors border-b-2 -mb-[1px] ${
        active
          ? "border-sky-500 text-white"
          : "border-transparent text-neutral-400 hover:text-neutral-100"
      }`}
    >
      {icon}
      {children}
    </button>
  );
}

function DetailTab({
  selectedKey,
  loading,
  issue,
  active,
  onStart,
  onStop,
}: {
  selectedKey: string | null;
  loading: boolean;
  issue: IssueRow | null | undefined;
  active: boolean;
  onStart: (key: string) => void;
  onStop: () => void;
}) {
  if (!selectedKey) {
    return (
      <div className="text-sm text-neutral-500 py-12 text-center">
        Pick an issue from the sidebar to see details and start tracking.
      </div>
    );
  }
  if (loading) {
    return (
      <div className="flex items-center justify-center py-6 text-neutral-500">
        <Spinner className="w-4 h-4 mr-2" />
        Loading {selectedKey}…
      </div>
    );
  }
  if (!issue) {
    return (
      <div className="text-sm text-neutral-500 py-12 text-center">
        <p>
          Issue <span className="font-mono">{selectedKey}</span> is not in the
          local cache.
        </p>
        <p className="mt-2 text-xs">
          Try clicking the sync button in the header.
        </p>
      </div>
    );
  }
  return (
    <IssueDetail
      issue={issue}
      active={active}
      onStart={onStart}
      onStop={onStop}
    />
  );
}

// -----------------------------------------------------------------------------
// helpers
// -----------------------------------------------------------------------------

function formatRow(row: WorklogRow): string {
  const minutes = Math.round(row.duration_s / 60);
  if (minutes < 1) return "<1m";
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

function formatMoney(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const rounded = Math.round(value * 100) / 100;
  return rounded.toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  });
}

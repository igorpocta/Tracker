/**
 * Today route — the default landing view.
 *
 * Sections:
 *   1. Big timer card (running/idle) with the current issue.
 *   2. Quick-start panel: search box + Recent / Suggested issues.
 *   3. Today's worklog list (from `get_worklogs_for_range`).
 *   4. Daily goal progress + estimated earnings.
 *
 * All data is sourced from the backend; we re-invalidate on the
 * `worklog-saved` and `auto-sync-complete` events (wired up in `AppShell`).
 */
import { useQuery } from "@tanstack/react-query";
import { Pencil, Play, Square } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useOutletContext } from "react-router-dom";

import {
  getRecentIssues,
  getSuggestedIssues,
  getWorklogsForRange,
  searchIssuesCache,
} from "../api/commands";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { Button } from "../components/common/Button";
import { Card } from "../components/common/Card";
import { IconButton } from "../components/common/IconButton";
import { Spinner } from "../components/common/Spinner";
import { DailyGoalBar } from "../components/Goal/DailyGoalBar";
import { GoalSettings } from "../components/Goal/GoalSettings";
import { IssueList } from "../components/Issues/IssueList";
import { SearchInput } from "../components/Issues/SearchInput";
import { Timer } from "../components/Timer/Timer";
import { WorklogList } from "../components/Worklog/WorklogList";
import { useNow } from "../hooks/useNow";
import {
  todayEndUnixS,
  todayStartUnixS,
} from "../lib/dates";
import { isToday as isTodayCheck } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";
import { elapsedSeconds, useTimerStore } from "../stores/timerStore";

export default function Today() {
  const ctx = useOutletContext<ShellOutletContext>();
  const active = useTimerStore((s) => s.active);
  const timerBusy = useTimerStore((s) => s.busy);
  const startTimerFn = useTimerStore((s) => s.start);
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);

  const [goalSettingsOpen, setGoalSettingsOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");

  // Debounce the search input by 150ms.
  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(search.trim()), 150);
    return () => window.clearTimeout(t);
  }, [search]);

  // ---- queries -------------------------------------------------------------
  const now = useNow(active ? 1000 : 60_000);

  // Snapshot today's range. Recomputed at most once a minute so the keys are
  // stable across renders (otherwise TanStack would refetch every tick).
  const todayRange = useMemo(() => {
    const reference = new Date(now);
    return [todayStartUnixS(reference), todayEndUnixS(reference)] as const;
  }, [Math.floor(now / 60_000)]);

  const todayWorklogsQ = useQuery({
    queryKey: ["worklogs-range", todayRange[0], todayRange[1]],
    queryFn: () => getWorklogsForRange(todayRange[0], todayRange[1]),
  });

  const recentQ = useQuery({
    queryKey: ["recent-issues", 10],
    queryFn: () => getRecentIssues(10),
  });
  const suggestedQ = useQuery({
    queryKey: ["suggested-issues", 6],
    queryFn: () => getSuggestedIssues(6),
  });
  const searchQ = useQuery({
    queryKey: ["search-issues", debounced],
    queryFn: () => searchIssuesCache(debounced, 20),
    enabled: debounced.length > 0,
  });

  // ---- derived totals ------------------------------------------------------
  const todaySeconds = useMemo(() => {
    const rows = todayWorklogsQ.data ?? [];
    const fromHistory = rows.reduce((acc, r) => acc + r.duration_s, 0);
    const fromActive =
      active && isTodayCheck(Math.floor(active.started_at / 1000), new Date(now))
        ? elapsedSeconds(active, now)
        : 0;
    return fromHistory + fromActive;
  }, [todayWorklogsQ.data, active, now]);

  const todayEarnings =
    hourlyRate > 0 ? (todaySeconds / 3600) * hourlyRate : 0;

  // ---- handlers ------------------------------------------------------------
  const handleStart = async (issueKey: string) => {
    try {
      await startTimerFn(issueKey);
    } catch (e) {
      ctx.pushToast("error", typeof e === "string" ? e : "Failed to start timer");
    }
  };

  // ---- render --------------------------------------------------------------
  return (
    <div className="p-6 flex flex-col gap-5 max-w-6xl mx-auto w-full">
      {/* Timer card -------------------------------------------------------- */}
      <Card padding="lg">
        <div className="flex items-center justify-between gap-6 flex-wrap">
          <div className="flex flex-col gap-1 min-w-0">
            <span className="text-[10px] uppercase tracking-wider text-neutral-500">
              {active ? "Tracking" : "Idle"}
            </span>
            <Timer className="text-6xl" />
            <span className="font-mono text-xs text-neutral-400 mt-1 truncate">
              {active ? active.issue_key : "Pick an issue to start a timer"}
            </span>
          </div>
          <div className="flex flex-col items-end gap-2">
            {active ? (
              <Button
                variant="danger"
                size="md"
                onClick={ctx.openStopDialog}
                disabled={timerBusy}
              >
                {timerBusy ? (
                  <Spinner className="w-3.5 h-3.5" />
                ) : (
                  <Square className="w-3.5 h-3.5" aria-hidden />
                )}
                Stop & save
              </Button>
            ) : (
              <span className="text-xs text-neutral-500">
                Use the list below to start a timer.
              </span>
            )}
          </div>
        </div>
      </Card>

      {/* Daily goal + earnings ------------------------------------------- */}
      <Card padding="md">
        <div className="flex items-start justify-between gap-4 flex-wrap">
          <div className="flex-1 min-w-[240px] max-w-md">
            <div className="flex items-center justify-between gap-2 mb-1.5">
              <h2 className="text-xs font-semibold uppercase tracking-wider text-neutral-400">
                Daily goal
              </h2>
              <IconButton
                aria-label="Edit daily goal"
                onClick={() => setGoalSettingsOpen((v) => !v)}
              >
                <Pencil className="w-3.5 h-3.5" aria-hidden />
              </IconButton>
            </div>
            <DailyGoalBar
              loggedSeconds={todaySeconds}
              goalSeconds={dailyGoalSeconds}
            />
            {hourlyRate > 0 && (
              <p className="text-xs text-neutral-400 mt-2">
                Estimated value:{" "}
                <span className="text-neutral-100 font-medium">
                  {formatMoney(todayEarnings)} {currency}
                </span>{" "}
                <span className="text-neutral-500">
                  ({hourlyRate}/h)
                </span>
              </p>
            )}
          </div>
        </div>
        {goalSettingsOpen && (
          <div className="mt-3 max-w-md">
            <GoalSettings
              open={goalSettingsOpen}
              onClose={() => setGoalSettingsOpen(false)}
            />
          </div>
        )}
      </Card>

      {/* Quick-start panel ----------------------------------------------- */}
      <Card padding="md">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-neutral-400 mb-3">
          Quick start
        </h2>
        <div className="mb-3">
          <SearchInput value={search} onChange={setSearch} placeholder="Search issues by key or summary…" />
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {debounced ? (
            <div className="md:col-span-2">
              <IssueList
                title={`Results for "${debounced}"`}
                issues={searchQ.data ?? []}
                loading={searchQ.isLoading || searchQ.isFetching}
                activeKey={active?.issue_key ?? null}
                onSelect={handleStart}
                emptyMessage="No matches found."
              />
            </div>
          ) : (
            <>
              <IssueList
                title="Suggested"
                issues={suggestedQ.data ?? []}
                loading={suggestedQ.isLoading}
                activeKey={active?.issue_key ?? null}
                onSelect={handleStart}
                emptyMessage="Track some time and we'll suggest issues here."
              />
              <IssueList
                title="Recent"
                issues={recentQ.data ?? []}
                loading={recentQ.isLoading}
                activeKey={active?.issue_key ?? null}
                onSelect={handleStart}
                emptyMessage="No recently updated issues. Try syncing."
              />
            </>
          )}
        </div>
        <p className="mt-2 text-[11px] text-neutral-500 flex items-center gap-1">
          <Play className="w-3 h-3" aria-hidden />
          Click any issue to start a timer immediately.
        </p>
      </Card>

      {/* Today's worklogs ------------------------------------------------ */}
      <Card padding="none">
        <div className="px-4 py-3 border-b border-neutral-800/70 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold">Today</h2>
            <p className="text-[11px] text-neutral-500">
              {todayWorklogsQ.data?.length ?? 0} entries ·{" "}
              <span className="text-neutral-300">
                {formatTotal(todaySeconds)} total
              </span>
            </p>
          </div>
        </div>
        <WorklogList
          rows={todayWorklogsQ.data}
          loading={todayWorklogsQ.isLoading}
          activeIssueKey={active?.issue_key ?? null}
          emptyTitle="No worklogs yet today"
          emptyDescription="Start your first timer above — entries will appear here as you log them."
        />
      </Card>
    </div>
  );
}

function formatMoney(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const rounded = Math.round(value * 100) / 100;
  return rounded.toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  });
}

function formatTotal(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h === 0 && m === 0) return "0m";
  if (h === 0) return `${m}m`;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

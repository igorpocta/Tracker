/**
 * Time Log route — the home screen.
 *
 * Reference: `screens/SCR-20260514-rjbm-2.png`.
 *
 *   Time Log  [Today ▾]  14/05/2026 → 14/05/2026                     5h 21m
 *                                                              Total duration
 *
 *   ┌─ Day timeline ───────────────────────────────────────────────────────┐
 *   │  06  07  08  09  ██  ██  12  ██  ██  15  16  ██  17  ██  19  20  21 │
 *   └────────────────────────────────────────────────────────────────────────┘
 *
 *   ┌─ DEV-792  Portál – Synchronizace…   14/05/2026  15:46 – 16:01   15m  🗑 ┐
 *   ├─ DEV-792  Portál – Synchronizace…   14/05/2026  13:00 – 15:08   2h 8m  ┤
 *   ├─ DEV-304  Úpravy ZZJ v OKO          14/05/2026  09:56 – 12:39   2h 43m ┤
 *   └─ DEV-926  Portal – (servis)…        14/05/2026  09:29 – 09:44   15m  🗑 ┘
 *
 *                                                              [ + New entry ]
 *
 * Period dropdown supports Today / Yesterday / This week. Date range is
 * read-only — picks the period's bounds. Rows are sorted descending so the
 * latest log floats to the top (reference behavior).
 */
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Plus, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useOutletContext } from "react-router-dom";

import { getWorklogsForRange } from "../api/commands";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { IssuePill } from "../components/common/IssuePill";
import { DayTimeline } from "../components/Timer/DayTimeline";
import {
  addDays,
  dayEndUnixS,
  dayStartUnixS,
  startOfDay,
  startOfWeek,
} from "../lib/dates";
import { formatDurationShort } from "../lib/format";

type Period = "today" | "yesterday" | "this-week";

const PERIOD_LABEL: Record<Period, string> = {
  today: "Today",
  yesterday: "Yesterday",
  "this-week": "This week",
};

export default function TimeLog() {
  const ctx = useOutletContext<ShellOutletContext>();
  const [period, setPeriod] = useState<Period>("today");

  const [from, to] = useMemo(() => periodRange(period), [period]);

  const fromUnix = dayStartUnixS(from);
  const toUnix = dayEndUnixS(to);

  const worklogsQ = useQuery({
    queryKey: ["worklogs-range", fromUnix, toUnix],
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = worklogsQ.data ?? [];
  const totalSeconds = rows.reduce((a, r) => a + r.duration_s, 0);

  return (
    <div className="px-6 pb-6 pt-2 flex flex-col gap-5 w-full max-w-[1100px] mx-auto">
      {/* Header row ----------------------------------------------------- */}
      <div className="flex items-baseline justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-baseline gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Time Log
          </h1>
          <PeriodSelector value={period} onChange={setPeriod} />
          <span className="text-xs font-mono text-[var(--text-tertiary)]">
            {formatDateShort(from)} → {formatDateShort(to)}
          </span>
        </div>
        <div className="text-right">
          <div className="text-xl font-semibold text-[var(--accent)] tabular-nums">
            {totalSeconds > 0 ? formatDurationShort(totalSeconds) : "0m"}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)]">
            Total duration
          </div>
        </div>
      </div>

      {/* Day timeline ---------------------------------------------------- */}
      <DayTimeline rows={rows} day={from} />

      {/* Worklog rows ---------------------------------------------------- */}
      <div className="flex flex-col gap-1">
        {worklogsQ.isLoading && (
          <div className="text-xs text-[var(--text-tertiary)] py-2">Loading…</div>
        )}
        {!worklogsQ.isLoading && rows.length === 0 && (
          <div className="text-xs text-[var(--text-tertiary)] py-6 text-center
                          rounded-[var(--radius-md)] border border-dashed border-[var(--border-subtle)]">
            No worklogs for this period. Press{" "}
            <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘N</kbd>{" "}
            to add one.
          </div>
        )}
        {[...rows]
          .sort((a, b) => b.started_at - a.started_at)
          .map((r) => (
            <WorklogRow key={r.id ?? `${r.issue_key}-${r.started_at}`} row={r} />
          ))}
      </div>

      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => ctx.openAddEntry?.()}
          className="inline-flex items-center gap-1.5 px-3.5 h-9
                     rounded-[var(--radius-md)] text-[13px] font-medium
                     bg-[var(--accent)] text-[var(--accent-text)]
                     hover:bg-[var(--accent-hover)]
                     transition-colors duration-150"
        >
          <Plus className="w-3.5 h-3.5" aria-hidden />
          New entry
        </button>
      </div>
    </div>
  );
}

function PeriodSelector({
  value,
  onChange,
}: {
  value: Period;
  onChange: (p: Period) => void;
}) {
  return (
    <label className="inline-flex items-center gap-1 cursor-pointer">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as Period)}
        className="appearance-none bg-transparent border-none text-sm text-[var(--text-secondary)]
                   cursor-pointer focus:outline-none pr-4"
        aria-label="Period"
      >
        {(["today", "yesterday", "this-week"] as Period[]).map((p) => (
          <option key={p} value={p}>
            {PERIOD_LABEL[p]}
          </option>
        ))}
      </select>
      <ChevronDown
        className="w-3 h-3 -ml-3 text-[var(--text-tertiary)] pointer-events-none"
        aria-hidden
      />
    </label>
  );
}

function WorklogRow({
  row,
}: {
  row: import("../api/types").WorklogRow;
}) {
  const started = new Date(row.started_at * 1000);
  const ended = new Date((row.started_at + row.duration_s) * 1000);

  return (
    <div
      className="flex items-center gap-3 h-12 px-3 rounded-[var(--radius-md)]
                 bg-[var(--bg-surface)] border border-[var(--border-subtle)]
                 hover:bg-[var(--bg-hover)] transition-colors duration-150"
    >
      <IssuePill issueKey={row.issue_key} />
      <span className="flex-1 min-w-0 truncate text-xs text-[var(--text-primary)]">
        {row.summary || "(no summary)"}
      </span>
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0
                       px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                       inline-flex items-center">
        {formatDateShort(started)}
      </span>
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0
                       px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                       inline-flex items-center">
        {formatHHMM(started)}
      </span>
      <span aria-hidden className="text-[var(--text-tertiary)]">–</span>
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0
                       px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                       inline-flex items-center">
        {formatHHMM(ended)}
      </span>
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-primary)] shrink-0 w-16 text-right">
        {formatDurationShort(row.duration_s)}
      </span>
      <button
        type="button"
        aria-label={`Delete worklog ${row.issue_key}`}
        title="Delete"
        className="text-[var(--text-tertiary)] hover:text-[var(--danger)] transition-colors duration-150"
      >
        <Trash2 className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}

function periodRange(p: Period): [Date, Date] {
  const today = startOfDay(new Date());
  if (p === "today") return [today, today];
  if (p === "yesterday") {
    const y = addDays(today, -1);
    return [y, y];
  }
  // This week
  const monday = startOfWeek(today);
  return [monday, today];
}

function formatDateShort(d: Date): string {
  const dd = `${d.getDate()}`.padStart(2, "0");
  const mm = `${d.getMonth() + 1}`.padStart(2, "0");
  const yyyy = d.getFullYear();
  return `${dd}/${mm}/${yyyy}`;
}

function formatHHMM(d: Date): string {
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

/**
 * Časový záznam (Time Log) — home screen.
 *
 *   Časový záznam  [Dnes ▾]  14. 5. 2026 → 14. 5. 2026                5h 21m
 *                                                                       Celkem
 *
 *   ┌─ Časová osa dne ────────────────────────────────────────────────────┐
 *   │  06  07  08  09  ██  ██  12  ██  ██  15  16  ██  17  ██  19  20  21 │
 *   └────────────────────────────────────────────────────────────────────────┘
 *
 *   ┌─ DEV-792  Portál – Synchronizace…   14. 5. 2026  15:46 – 16:01  15m ┐
 *   …
 *
 * The DayTimeline is rendered only when the user keeps it visible
 * (Nastavení → Obecné → Časová osa dne). The pref is now backend-backed
 * and read from the prefs store.
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
import { formatDateCs, formatDurationShort } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";

type Period = "today" | "yesterday" | "this-week";

const PERIOD_LABEL: Record<Period, string> = {
  today: "Dnes",
  yesterday: "Včera",
  "this-week": "Tento týden",
};

export default function TimeLog() {
  const ctx = useOutletContext<ShellOutletContext>();
  const [period, setPeriod] = useState<Period>("today");
  const dayTimelineVisible = usePrefsStore((s) => s.dayTimelineVisible);

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
            Časový záznam
          </h1>
          <PeriodSelector value={period} onChange={setPeriod} />
          <span className="text-xs font-mono text-[var(--text-tertiary)]">
            {formatDateCs(from)} → {formatDateCs(to)}
          </span>
        </div>
        <div className="text-right">
          <div className="text-xl font-semibold text-[var(--accent)] tabular-nums">
            {totalSeconds > 0 ? formatDurationShort(totalSeconds) : "0m"}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)]">
            Celkem
          </div>
        </div>
      </div>

      {/* Day timeline (optional, user pref) ----------------------------- */}
      {dayTimelineVisible && <DayTimeline rows={rows} day={from} />}

      {/* Worklog rows ---------------------------------------------------- */}
      <div className="flex flex-col gap-1">
        {worklogsQ.isLoading && (
          <div className="text-xs text-[var(--text-tertiary)] py-2">Načítání…</div>
        )}
        {!worklogsQ.isLoading && rows.length === 0 && (
          <div className="text-xs text-[var(--text-tertiary)] py-6 text-center
                          rounded-[var(--radius-md)] border border-dashed border-[var(--border-subtle)]">
            Pro toto období nejsou žádné záznamy. Stiskněte{" "}
            <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘N</kbd>{" "}
            pro přidání.
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
          Nový záznam
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
        aria-label="Období"
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
        {row.summary || "(bez popisu)"}
      </span>
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0
                       px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                       inline-flex items-center">
        {formatDateCs(started)}
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
        aria-label={`Smazat záznam ${row.issue_key}`}
        title="Smazat"
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

function formatHHMM(d: Date): string {
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

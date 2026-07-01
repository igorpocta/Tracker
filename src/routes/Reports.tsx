/**
 * Reports route — period summary + daily bar chart + issues breakdown.
 *
 * Reference: `screens/SCR-20260514-rjcn-2.png`.
 *
 *   Reports  This month ▾  01/05/2026 → 31/05/2026          [↓ Export XLSX]
 *
 *   ┌ Total time   ┐ ┌ Days worked ┐ ┌ Issues touched ┐ ┌ Earnings ⊙ ┐
 *   │ 39h 24m      │ │ 7           │ │ 17             │ │ ••••        │
 *   └──────────────┘ └─────────────┘ └────────────────┘ └─────────────┘
 *
 *   ┌─ Daily hours ─────────────────────────────────────────────────────┐
 *   │ 12 ┤                                                              │
 *   │  9 ┤       █                                                       │
 *   │  6 ┤       █  █                  █                                 │
 *   │  3 ┤    █  █  █              █   █                                 │
 *   │  0 ┴─01/05─03/05─05/05─07/05─11/05─13/05────────────────────────  │
 *   └────────────────────────────────────────────────────────────────────┘
 *
 *   Issues breakdown
 *   Issue     Summary               Total    Last logged
 *   DEV-304   Úpravy ZZJ v OKO      20h 10m  14/05/2026
 *   …
 *
 * The "Earnings" card is hidden by default behind a click-to-reveal icon —
 * useful in shared screens / open spaces. When the hourly rate is 0 the
 * card shows a dash.
 */
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Flame } from "lucide-react";
import { useMemo, useState } from "react";

import { getStreaks, getWorklogsForRange } from "../api/commands";
import { queryKeys } from "../api/queryKeys";
import type { WorklogRow } from "../api/types";
import { IssuePill } from "../components/common/IssuePill";
import { PageContainer } from "../components/Layout/PageContainer";
import { DailyBarChart } from "../components/Reports/DailyBarChart";
import { ExportButton } from "../components/Reports/ExportButton";
import { SummaryCards } from "../components/Reports/SummaryCards";
import {
  addDays,
  dayEndUnixS,
  dayOverlapSeconds,
  dayStartUnixS,
  endOfMonth,
  endOfPreviousMonth,
  startOfDay,
  startOfMonth,
  startOfPreviousMonth,
  startOfWeek,
  startOfYear,
} from "../lib/dates";
import { formatDateCs, formatDurationShort } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";

type Period =
  | "this-week"
  | "last-week"
  | "this-month"
  | "last-month"
  | "last-30"
  | "this-year";

const PERIOD_LABEL: Record<Period, string> = {
  "this-week": "Tento týden",
  "last-week": "Minulý týden",
  "this-month": "Tento měsíc",
  "last-month": "Minulý měsíc",
  "last-30": "Posledních 30 dní",
  "this-year": "Od začátku roku",
};

export default function Reports() {
  const [period, setPeriod] = useState<Period>("this-month");
  const [from, to] = useMemo(() => periodRange(period), [period]);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const dailyGoalHours = dailyGoalSeconds / 3600;

  const fromUnix = dayStartUnixS(from);
  const toUnix = dayEndUnixS(to);

  const q = useQuery({
    queryKey: queryKeys.worklogs.range(fromUnix, toUnix),
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const streakQ = useQuery({
    queryKey: ["streaks"],
    queryFn: getStreaks,
    staleTime: 60_000,
  });

  // Stable identity so the derived useMemos below don't recompute every render.
  const rows = useMemo(() => q.data ?? [], [q.data]);
  // The fetch returns worklogs OVERLAPPING [fromUnix, toUnix] (variant B), so
  // clip each row to the period before summing — otherwise a worklog straddling
  // the period boundary would over-count its out-of-period slice.
  const toUnixExcl = toUnix + 1;
  const totalSeconds = rows.reduce(
    (a, r) =>
      a +
      dayOverlapSeconds(
        r.started_at,
        r.ended_at ?? r.started_at + r.duration_s,
        fromUnix,
        toUnixExcl,
      ),
    0,
  );
  const daysWorked = useMemo(
    () => uniqueDays(rows, fromUnix, toUnix),
    [rows, fromUnix, toUnix],
  );
  const issuesTouched = useMemo(
    () => new Set(rows.map((r) => r.issue_key)).size,
    [rows],
  );
  const earnings = hourlyRate > 0 ? (totalSeconds / 3600) * hourlyRate : 0;

  return (
    <PageContainer>
      <div className="flex items-baseline justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-baseline gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Reporty
          </h1>
          <PeriodSelector value={period} onChange={setPeriod} />
          <span className="text-xs font-mono text-[var(--text-tertiary)]">
            {formatDateCs(from)} → {formatDateCs(to)}
          </span>
        </div>
        <div className="flex items-center gap-3 flex-wrap">
          <StreakBadge streaks={streakQ.data} />
          <ExportButton rows={rows} from={from} to={to} />
        </div>
      </div>

      <SummaryCards
        totalSeconds={totalSeconds}
        daysWorked={daysWorked}
        issuesTouched={issuesTouched}
        earnings={earnings}
        currency={currency}
        hourlyRateConfigured={hourlyRate > 0}
        durationLabel={
          totalSeconds > 0 ? (
            <span className="text-[var(--accent)]">
              {formatDurationShort(totalSeconds)}
            </span>
          ) : (
            <span className="text-[var(--text-tertiary)]">0m</span>
          )
        }
      />

      <DailyBarChart
        rows={rows}
        from={from}
        to={to}
        dailyGoalHours={dailyGoalHours}
      />

      <IssuesBreakdown rows={rows} fromUnix={fromUnix} toUnixExcl={toUnixExcl} />
    </PageContainer>
  );
}

// -----------------------------------------------------------------------------
// Subcomponents
// -----------------------------------------------------------------------------

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
        {(
          [
            "this-week",
            "last-week",
            "this-month",
            "last-month",
            "last-30",
            "this-year",
          ] as Period[]
        ).map((p) => (
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

function StreakBadge({
  streaks,
}: {
  streaks?: { current: number; longest: number; today_met: boolean };
}) {
  if (!streaks || streaks.current === 0) return null;
  const days = streaks.current;
  // "Pracovní dny" pluralizace pro češtinu.
  const label = days === 1 ? "den" : days >= 2 && days <= 4 ? "dny" : "dní";
  const isRecord = streaks.current === streaks.longest;
  const tooltip = isRecord
    ? "Po sobě jdoucí pracovní dny se splněným denním cílem · osobní rekord!"
    : `Po sobě jdoucí pracovní dny se splněným denním cílem · nejdelší ${streaks.longest}`;
  return (
    <div
      title={tooltip}
      className="inline-flex items-center gap-1.5 px-2.5 h-8 rounded-full text-xs font-medium"
      style={{
        background: "var(--accent-soft)",
        color: "var(--accent)",
      }}
    >
      <Flame className="w-3.5 h-3.5" aria-hidden />
      <span>
        {days} {label}
      </span>
      {!isRecord && (
        // Secondary tint marks the all-time peak so the current count stays
        // the visual primary in the chip. In mono palettes accent-2 == accent,
        // so this renders identically there; only dual palettes show the pair.
        <span className="text-[10px]" style={{ color: "var(--accent-2)" }}>
          · rekord {streaks.longest}
        </span>
      )}
      {!streaks.today_met && (
        <span className="text-[10px] opacity-60">· dnes ještě</span>
      )}
    </div>
  );
}

function IssuesBreakdown({
  rows,
  fromUnix,
  toUnixExcl,
}: {
  rows: WorklogRow[];
  fromUnix: number;
  toUnixExcl: number;
}) {
  const aggregated = useMemo(
    () => aggregateByIssue(rows, fromUnix, toUnixExcl),
    [rows, fromUnix, toUnixExcl],
  );
  const [hoverKey, setHoverKey] = useState<string | null>(null);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
        Rozpad úkolů
      </h3>
      {/* Hlavička je samostatný grid se stejnou template column definicí
          jako řádky níž — díky `grid-cols-subgrid` na řádcích zůstanou
          sloupce zarovnané přes celou tabulku.                       */}
      <div className="grid grid-cols-[auto_1fr_auto_auto] gap-x-4 text-xs px-2 -mx-2">
        <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] pb-1">
          Úkol
        </div>
        <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] pb-1">
          Popis
        </div>
        <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] pb-1 text-right">
          Celkem
        </div>
        <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)] pb-1 text-right">
          Naposledy zaznamenáno
        </div>
      </div>
      {aggregated.length === 0 ? (
        <div className="py-4 text-center text-[var(--text-tertiary)] text-xs">
          Zatím prázdné.
        </div>
      ) : (
        <div className="grid grid-cols-[auto_1fr_auto_auto] gap-x-4 text-xs">
          {aggregated.map((a) => {
            const isHovered = hoverKey === a.issueKey;
            return (
              <div
                key={a.issueKey}
                className="col-span-4 grid grid-cols-subgrid gap-x-4 items-center min-h-[32px] px-2 -mx-2 rounded-[6px]"
                style={{
                  // Sekundární accent na hoveru: v mono paletě splývá s
                  // primárním (accent-2 == accent), v dual paletě jasně
                  // odlišuje hover od stavu "klik otevírá ten samý úkol".
                  background: isHovered ? "var(--accent-2-soft)" : "transparent",
                  transition: "background-color 120ms ease-out",
                }}
                onMouseEnter={() => setHoverKey(a.issueKey)}
                onMouseLeave={() => setHoverKey(null)}
              >
                <div className="flex items-center">
                  <IssuePill issueKey={a.issueKey} />
                </div>
                <div className="truncate text-[var(--text-secondary)]">
                  {a.summary || "(načítá se…)"}
                </div>
                <div className="text-right font-mono tabular-nums text-[var(--text-primary)]">
                  {formatDurationShort(a.totalSeconds)}
                </div>
                <div className="text-right font-mono tabular-nums text-[var(--text-tertiary)]">
                  {formatDateCs(new Date(a.lastLoggedUnixS * 1000))}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function uniqueDays(rows: WorklogRow[], fromUnix: number, toUnix: number): number {
  const s = new Set<string>();
  const toExcl = toUnix + 1;
  for (const r of rows) {
    // Count every local day the worklog actually overlaps WITHIN the period —
    // half-open, variant B. A cross-midnight entry counts both days; a row that
    // only overflowed in from an earlier day counts just the in-period day(s).
    const endedAt = r.ended_at ?? r.started_at + r.duration_s;
    let dayDate = startOfDay(new Date(r.started_at * 1000));
    const lastDay = startOfDay(new Date(endedAt * 1000));
    while (dayDate.getTime() <= lastDay.getTime()) {
      const dayStart = Math.max(dayStartUnixS(dayDate), fromUnix);
      const dayEnd = Math.min(dayStartUnixS(addDays(dayDate, 1)), toExcl);
      if (dayOverlapSeconds(r.started_at, endedAt, dayStart, dayEnd) > 0) {
        s.add(formatKey(dayDate));
      }
      dayDate = addDays(dayDate, 1);
    }
  }
  return s.size;
}

interface Aggregated {
  issueKey: string;
  summary: string | null | undefined;
  totalSeconds: number;
  lastLoggedUnixS: number;
}

function aggregateByIssue(
  rows: WorklogRow[],
  fromUnix: number,
  toUnixExcl: number,
): Aggregated[] {
  const map = new Map<string, Aggregated>();
  for (const r of rows) {
    // Clip to the period (overlap-based fetch — variant B) so an edge worklog
    // contributes only its in-period seconds to the issue total.
    const secs = dayOverlapSeconds(
      r.started_at,
      r.ended_at ?? r.started_at + r.duration_s,
      fromUnix,
      toUnixExcl,
    );
    if (secs === 0) continue;
    // Local-only worklogy bez přiřazeného úkolu drží `null`. Zařadíme je
    // pod sentinelový klíč, aby se nesloučily s jinými.
    const key = r.issue_key ?? "(bez úkolu)";
    const cur = map.get(key);
    if (!cur) {
      map.set(key, {
        issueKey: key,
        summary: r.summary,
        totalSeconds: secs,
        lastLoggedUnixS: r.started_at,
      });
    } else {
      cur.totalSeconds += secs;
      if (r.started_at > cur.lastLoggedUnixS) {
        cur.lastLoggedUnixS = r.started_at;
        cur.summary = r.summary ?? cur.summary;
      }
    }
  }
  return Array.from(map.values()).sort((a, b) => b.totalSeconds - a.totalSeconds);
}

function periodRange(p: Period): [Date, Date] {
  const today = startOfDay(new Date());
  if (p === "this-week") {
    return [startOfWeek(today), today];
  }
  if (p === "last-week") {
    const start = addDays(startOfWeek(today), -7);
    return [start, addDays(start, 6)];
  }
  if (p === "this-month") {
    return [startOfMonth(today), endOfMonth(today)];
  }
  if (p === "last-month") {
    return [startOfPreviousMonth(today), endOfPreviousMonth(today)];
  }
  if (p === "this-year") {
    return [startOfYear(today), today];
  }
  // last-30
  return [addDays(today, -29), today];
}

function formatKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

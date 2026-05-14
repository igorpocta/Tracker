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
import { ChevronDown, Download, Eye, EyeOff } from "lucide-react";
import { useMemo, useState } from "react";

import { getWorklogsForRange } from "../api/commands";
import type { WorklogRow } from "../api/types";
import { IssuePill } from "../components/common/IssuePill";
import {
  addDays,
  dayEndUnixS,
  dayStartUnixS,
  endOfMonth,
  endOfPreviousMonth,
  startOfDay,
  startOfMonth,
  startOfPreviousMonth,
  startOfWeek,
} from "../lib/dates";
import { formatDateCs, formatDateCsShort, formatDurationShort, formatMoney } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";

type Period = "this-week" | "last-week" | "this-month" | "last-month" | "last-30";

const PERIOD_LABEL: Record<Period, string> = {
  "this-week": "Tento týden",
  "last-week": "Minulý týden",
  "this-month": "Tento měsíc",
  "last-month": "Minulý měsíc",
  "last-30": "Posledních 30 dní",
};

export default function Reports() {
  const [period, setPeriod] = useState<Period>("this-month");
  const [from, to] = useMemo(() => periodRange(period), [period]);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);

  const fromUnix = dayStartUnixS(from);
  const toUnix = dayEndUnixS(to);

  const q = useQuery({
    queryKey: ["worklogs-range", fromUnix, toUnix],
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = q.data ?? [];
  const totalSeconds = rows.reduce((a, r) => a + r.duration_s, 0);
  const daysWorked = useMemo(() => uniqueDays(rows), [rows]);
  const issuesTouched = useMemo(
    () => new Set(rows.map((r) => r.issue_key)).size,
    [rows],
  );
  const earnings = hourlyRate > 0 ? (totalSeconds / 3600) * hourlyRate : 0;

  return (
    <div className="px-6 pb-6 pt-2 flex flex-col gap-5 w-full max-w-[1100px] mx-auto">
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
        <button
          type="button"
          onClick={() => exportXlsx(rows, from, to)}
          className="inline-flex items-center gap-1.5 px-3 h-8
                     rounded-[var(--radius-md)] text-xs text-[var(--accent)]
                     border border-[var(--accent-soft)]
                     bg-transparent hover:bg-[var(--accent-soft)]
                     transition-colors duration-150"
        >
          <Download className="w-3.5 h-3.5" aria-hidden />
          Exportovat XLSX
        </button>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
        <BigStatCard
          label="Celkový čas"
          value={
            totalSeconds > 0 ? (
              <span className="text-[var(--accent)]">
                {formatDurationShort(totalSeconds)}
              </span>
            ) : (
              <span className="text-[var(--text-tertiary)]">0m</span>
            )
          }
        />
        <BigStatCard label="Odpracovaných dní" value={`${daysWorked}`} />
        <BigStatCard label="Dotčených úkolů" value={`${issuesTouched}`} />
        <EarningsCard
          earnings={earnings}
          currency={currency}
          enabled={hourlyRate > 0}
        />
      </div>

      <DailyHoursChart rows={rows} from={from} to={to} />

      <IssuesBreakdown rows={rows} />
    </div>
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
        {(["this-week", "last-week", "this-month", "last-month", "last-30"] as Period[]).map((p) => (
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

function BigStatCard({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-4">
      <div className="text-[11px] text-[var(--text-tertiary)]">{label}</div>
      <div className="mt-2 text-2xl font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function EarningsCard({
  earnings,
  currency,
  enabled,
}: {
  earnings: number;
  currency: string;
  enabled: boolean;
}) {
  const [revealed, setRevealed] = useState(false);
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-4 relative">
      <div className="flex items-center justify-between">
        <div className="text-[11px] text-[var(--text-tertiary)]">Výdělek</div>
        {enabled && (
          <button
            type="button"
            onClick={() => setRevealed((v) => !v)}
            aria-label={revealed ? "Skrýt výdělek" : "Zobrazit výdělek"}
            className="text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]
                       transition-colors duration-150"
          >
            {revealed ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
          </button>
        )}
      </div>
      <div className="mt-2 text-2xl font-semibold tabular-nums text-[var(--accent-2)]">
        {!enabled ? (
          <span className="text-[var(--text-tertiary)]">—</span>
        ) : revealed ? (
          formatMoney(earnings, currency)
        ) : (
          <span aria-hidden className="tracking-wider">
            ••••
          </span>
        )}
      </div>
    </div>
  );
}

function DailyHoursChart({
  rows,
  from,
  to,
}: {
  rows: WorklogRow[];
  from: Date;
  to: Date;
}) {
  const days = useMemo(() => buildDayList(from, to), [from, to]);
  const totals = useMemo(() => totalsByDay(rows), [rows]);

  const max = useMemo(() => {
    let m = 0;
    for (const d of days) {
      const v = (totals.get(formatKey(d)) ?? 0) / 3600;
      if (v > m) m = v;
    }
    return Math.max(3, Math.ceil(m / 3) * 3);
  }, [days, totals]);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
        Hodiny za den
      </h3>
      <div className="flex gap-4 h-[260px]">
        <div className="flex flex-col justify-between text-[10px] text-[var(--text-tertiary)] tabular-nums py-1">
          {[max, Math.round((max * 2) / 3), Math.round(max / 3), 0].map((v) => (
            <div key={`y-${v}`}>{v}</div>
          ))}
        </div>
        <div className="flex-1 relative">
          <div className="absolute inset-0 flex flex-col justify-between py-1 pointer-events-none">
            {[0, 1, 2, 3].map((i) => (
              <div
                key={`grid-${i}`}
                className="border-t border-[var(--border-subtle)]"
              />
            ))}
          </div>
          <div className="absolute inset-0 flex items-end gap-[2px] pt-1 pb-5">
            {days.map((d) => {
              const seconds = totals.get(formatKey(d)) ?? 0;
              const hours = seconds / 3600;
              const heightPct = max > 0 ? (hours / max) * 100 : 0;
              return (
                <div
                  key={d.toISOString()}
                  className="flex-1 relative"
                  title={`${formatDateCs(d)} · ${formatDurationShort(seconds)}`}
                >
                  <div
                    className="absolute left-0 right-0 bottom-0 rounded-t-[2px]"
                    style={{
                      height: `${heightPct}%`,
                      minHeight: heightPct > 0 ? "2px" : 0,
                      background:
                        heightPct > 0
                          ? "var(--accent)"
                          : "var(--bg-active)",
                    }}
                  />
                </div>
              );
            })}
          </div>
          <div className="absolute left-0 right-0 bottom-0 flex gap-[2px]">
            {days.map((d, idx) => (
              <div
                key={`label-${d.toISOString()}`}
                className="flex-1 text-[9px] text-[var(--text-tertiary)] text-center tabular-nums"
              >
                {idx % 2 === 0 ? formatDateCsShort(d) : ""}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function IssuesBreakdown({ rows }: { rows: WorklogRow[] }) {
  const aggregated = useMemo(() => aggregateByIssue(rows), [rows]);

  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
        Rozpad úkolů
      </h3>
      <div className="grid grid-cols-[auto_1fr_auto_auto] gap-x-4 gap-y-1 text-xs items-center">
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
        {aggregated.length === 0 && (
          <div className="col-span-4 py-4 text-center text-[var(--text-tertiary)]">
            Zatím prázdné.
          </div>
        )}
        {aggregated.map((a) => (
          <div className="contents" key={a.issueKey}>
            <div>
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
        ))}
      </div>
    </div>
  );
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function uniqueDays(rows: WorklogRow[]): number {
  const s = new Set<string>();
  for (const r of rows) {
    const d = new Date(r.started_at * 1000);
    s.add(formatKey(d));
  }
  return s.size;
}

function totalsByDay(rows: WorklogRow[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const r of rows) {
    const d = new Date(r.started_at * 1000);
    const k = formatKey(d);
    map.set(k, (map.get(k) ?? 0) + r.duration_s);
  }
  return map;
}

function buildDayList(from: Date, to: Date): Date[] {
  const out: Date[] = [];
  const d = startOfDay(from);
  const end = startOfDay(to);
  while (d <= end) {
    out.push(new Date(d));
    d.setDate(d.getDate() + 1);
  }
  return out;
}

interface Aggregated {
  issueKey: string;
  summary: string | null | undefined;
  totalSeconds: number;
  lastLoggedUnixS: number;
}

function aggregateByIssue(rows: WorklogRow[]): Aggregated[] {
  const map = new Map<string, Aggregated>();
  for (const r of rows) {
    const cur = map.get(r.issue_key);
    if (!cur) {
      map.set(r.issue_key, {
        issueKey: r.issue_key,
        summary: r.summary,
        totalSeconds: r.duration_s,
        lastLoggedUnixS: r.started_at,
      });
    } else {
      cur.totalSeconds += r.duration_s;
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
  // last-30
  return [addDays(today, -29), today];
}

function formatKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}


/**
 * Export the supplied worklog rows as a CSV download. Despite the button
 * label ("Export XLSX") we ship CSV today — Excel opens CSV happily, and
 * adding a real XLSX writer dependency for one button is poor leverage.
 */
function exportXlsx(rows: WorklogRow[], from: Date, to: Date): void {
  const headers = ["Úkol", "Popis", "Datum", "Začátek", "Konec", "Trvání (min)", "Komentář"];
  const body = rows.map((r) => {
    const start = new Date(r.started_at * 1000);
    const end = new Date((r.started_at + r.duration_s) * 1000);
    return [
      r.issue_key,
      r.summary ?? "",
      formatDateCs(start),
      `${`${start.getHours()}`.padStart(2, "0")}:${`${start.getMinutes()}`.padStart(2, "0")}`,
      `${`${end.getHours()}`.padStart(2, "0")}:${`${end.getMinutes()}`.padStart(2, "0")}`,
      `${Math.round(r.duration_s / 60)}`,
      r.comment ?? "",
    ];
  });
  const csv = [headers, ...body]
    .map((row) =>
      row
        .map((cell) => {
          const s = `${cell ?? ""}`;
          return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
        })
        .join(","),
    )
    .join("\n");

  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `tracker-${formatIso(from)}-${formatIso(to)}.csv`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function formatIso(d: Date): string {
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

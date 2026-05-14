/**
 * Calendar overview — monthly grid of days, tinted by hours logged.
 *
 * Reference: `screens/SCR-20260514-rjem-2.png`.
 *
 *  Calendar overview  [Monthly | Yearly]  2026 ▾  May ▾                39h 24m
 *                                                                   Month total
 *
 *  MON  TUE  WED  THU  FRI  SAT  SUN
 *                       01   02   03
 *  04   ██   ██   ██    08   09   10
 *  11   ██   ██   ██    15   16   17
 *  18   19   20   21    22   23   24
 *  25   26   27   28    29   30   31
 *
 * Each cell shows the day number (top-left) and the hours logged (bottom-left,
 * "6.9h" / "—"). The background fills with accent-soft proportional to fill,
 * so the active days form a "heat" pattern. Click a day to load its detail
 * panel on the right (Phase 13B; for now the click is a no-op).
 */
import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { useMemo, useState } from "react";

import { getWorklogsForRange } from "../api/commands";
import type { WorklogRow } from "../api/types";
import {
  addDays,
  dayEndUnixS,
  dayStartUnixS,
  endOfMonth,
  isSameDay,
  startOfDay,
  startOfMonth,
} from "../lib/dates";
import { formatDurationShort, formatHours } from "../lib/format";

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

export default function Calendar() {
  const today = useMemo(() => startOfDay(new Date()), []);
  const [year, setYear] = useState(today.getFullYear());
  const [month, setMonth] = useState(today.getMonth());
  const [view, setView] = useState<"monthly" | "yearly">("monthly");

  const monthStart = useMemo(
    () => startOfMonth(new Date(year, month, 1)),
    [year, month],
  );
  const monthEnd = useMemo(() => endOfMonth(monthStart), [monthStart]);

  const fromUnix = dayStartUnixS(monthStart);
  const toUnix = dayEndUnixS(monthEnd);

  const q = useQuery({
    queryKey: ["worklogs-range", fromUnix, toUnix],
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = q.data ?? [];
  const dayTotals = useMemo(() => totalsByDay(rows), [rows]);
  const monthTotal = rows.reduce((a, r) => a + r.duration_s, 0);

  // Build the calendar cells: pad leading days from the previous month with
  // empty placeholders so the grid starts on Monday.
  const monthDays: Date[] = [];
  for (let i = 0; i < monthEnd.getDate(); i++) {
    monthDays.push(addDays(monthStart, i));
  }
  // JS getDay: Sunday=0. We want Monday=0.
  const leadingBlanks = (monthStart.getDay() + 6) % 7;

  return (
    <div className="px-6 pb-6 pt-2 flex flex-col gap-5 w-full max-w-[1100px] mx-auto">
      <div className="flex items-baseline justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-baseline gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Calendar overview
          </h1>
          <Segmented
            value={view}
            onChange={setView}
            options={[
              { value: "monthly", label: "Monthly" },
              { value: "yearly", label: "Yearly" },
            ]}
          />
          <YearMonthPickers
            year={year}
            month={month}
            onYearChange={setYear}
            onMonthChange={setMonth}
          />
        </div>
        <div className="text-right">
          <div className="text-xl font-semibold text-[var(--accent)] tabular-nums">
            {monthTotal > 0 ? formatDurationShort(monthTotal) : "0m"}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)]">
            Month total
          </div>
        </div>
      </div>

      {/* Weekday header */}
      <div className="grid grid-cols-7 gap-2 px-1">
        {["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"].map((d) => (
          <div
            key={d}
            className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]"
          >
            {d}
          </div>
        ))}
      </div>

      <div className="grid grid-cols-7 gap-2">
        {Array.from({ length: leadingBlanks }).map((_, i) => (
          <div key={`blank-${i}`} />
        ))}
        {monthDays.map((d) => {
          const seconds = dayTotals.get(formatKey(d)) ?? 0;
          const isToday = isSameDay(d, today);
          return (
            <CalendarCell
              key={d.toISOString()}
              date={d}
              seconds={seconds}
              isToday={isToday}
            />
          );
        })}
      </div>
    </div>
  );
}

function CalendarCell({
  date,
  seconds,
  isToday,
}: {
  date: Date;
  seconds: number;
  isToday: boolean;
}) {
  const hours = seconds / 3600;
  const fill = Math.min(1, hours / 8);
  const filled = hours > 0;
  return (
    <button
      type="button"
      className={clsx(
        "relative h-[88px] rounded-[var(--radius-md)] p-2 text-left",
        "border transition-colors duration-150",
        filled
          ? "border-transparent"
          : "border-[var(--border-subtle)] bg-[var(--bg-surface)]",
        isToday && !filled && "ring-1 ring-[var(--accent)]",
      )}
      style={
        filled
          ? {
              background: `color-mix(in srgb, var(--accent) ${Math.round(
                20 + fill * 60,
              )}%, var(--bg-surface))`,
              color: "var(--text-primary)",
            }
          : undefined
      }
    >
      <div className="text-xs font-medium text-[var(--text-primary)]">
        {`${date.getDate()}`.padStart(2, "0")}
      </div>
      <div className="absolute left-2 bottom-2 text-xs">
        {filled ? (
          <span className={isToday ? "text-[var(--accent)]" : "text-[var(--text-primary)]"}>
            {formatHours(hours)}
          </span>
        ) : (
          <span className="text-[var(--text-tertiary)]">—</span>
        )}
      </div>
    </button>
  );
}

function Segmented<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
}) {
  return (
    <div className="inline-flex items-center rounded-full border border-[var(--border-subtle)] p-0.5 text-xs">
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          onClick={() => onChange(opt.value)}
          className={clsx(
            "px-3 h-6 rounded-full transition-colors duration-150",
            value === opt.value
              ? "bg-[var(--accent-soft)] text-[var(--accent)]"
              : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]",
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

function YearMonthPickers({
  year,
  month,
  onYearChange,
  onMonthChange,
}: {
  year: number;
  month: number;
  onYearChange: (y: number) => void;
  onMonthChange: (m: number) => void;
}) {
  const years = useMemo(() => {
    const now = new Date().getFullYear();
    return Array.from({ length: 6 }, (_, i) => now - 3 + i);
  }, []);

  return (
    <div className="inline-flex items-center gap-2 text-sm text-[var(--text-secondary)]">
      <select
        value={year}
        onChange={(e) => onYearChange(parseInt(e.target.value, 10))}
        className="appearance-none bg-transparent border-none cursor-pointer focus:outline-none"
        aria-label="Year"
      >
        {years.map((y) => (
          <option key={y} value={y}>
            {y}
          </option>
        ))}
      </select>
      <select
        value={month}
        onChange={(e) => onMonthChange(parseInt(e.target.value, 10))}
        className="appearance-none bg-transparent border-none cursor-pointer focus:outline-none"
        aria-label="Month"
      >
        {MONTHS.map((m, i) => (
          <option key={m} value={i}>
            {m}
          </option>
        ))}
      </select>
    </div>
  );
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

function formatKey(d: Date): string {
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

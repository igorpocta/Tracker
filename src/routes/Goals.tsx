/**
 * Goals route — today/month progress + 4 stat cards.
 *
 * Reference: `screens/SCR-20260514-rjfw-2.png` and `rlku-2.png`.
 *
 *  Goals                                                10 / 21 work days
 *  May 2026 · 9h daily goal                                         elapsed
 *
 *  ┌─ Today ───────────────────────────────────────────── 5h 21m ─┐
 *  │ Thursday, 14 May                                     goal: 9h │
 *  │ ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
 *  │ 5h 21m / 9h                                                59% │
 *  └────────────────────────────────────────────────────────────────┘
 *
 *  ┌─ This month ──────────────────────────────────────── 39h 24m ┐
 *  │ 21 working days · 9h each = 189h total              goal: 189h│
 *  │ █████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
 *  │ 39h 24m / 189h                                              21% │
 *  └────────────────────────────────────────────────────────────────┘
 *
 *  ┌ Expected today ┐ ┌ Actual logged ┐ ┌ Pace diff ┐ ┌ Remaining ┐
 *  │ 90h 0m         │ │ 39h 24m       │ │ −50h 36m  │ │ 11        │
 *  └────────────────┘ └────────────────┘ └───────────┘ └───────────┘
 *
 * Working-day math: Mon–Fri only. The "Expected today" assumes you log
 * `dailyGoal` on every working day from month start through today
 * (inclusive). "Pace difference" = Actual − Expected (negative means behind).
 */
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { getWorklogsForRange } from "../api/commands";
import { queryKeys } from "../api/queryKeys";
import { GoalsPrediction } from "../components/Goals/GoalsPrediction";
import { PageContainer } from "../components/Layout/PageContainer";
import { useCalendarMask, isWorkingDayLocal } from "../hooks/useCalendarMask";
import { useTodayBoundary } from "../hooks/useTodayBoundary";
import {
  addDays,
  dayEndUnixS,
  dayOverlapSeconds,
  dayStartUnixS,
  endOfMonth,
  startOfDay,
  startOfMonth,
} from "../lib/dates";
import { formatDurationShort } from "../lib/format";
import { usePrefsStore } from "../stores/prefsStore";

const MONTHS_LONG = [
  "leden",
  "únor",
  "březen",
  "duben",
  "květen",
  "červen",
  "červenec",
  "srpen",
  "září",
  "říjen",
  "listopad",
  "prosinec",
];

const WEEKDAYS_LONG = [
  "Neděle",
  "Pondělí",
  "Úterý",
  "Středa",
  "Čtvrtek",
  "Pátek",
  "Sobota",
];

export default function Goals() {
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  // Recompute `today` (and the month range derived from it) on day rollover.
  const { rolloverCount } = useTodayBoundary();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const today = useMemo(() => startOfDay(new Date()), [rolloverCount]);
  const monthStart = useMemo(() => startOfMonth(today), [today]);
  const monthEnd = useMemo(() => endOfMonth(today), [today]);

  const dailyGoalHours = dailyGoalSeconds / 3600;

  const fromUnix = dayStartUnixS(monthStart);
  const toUnix = dayEndUnixS(monthEnd);

  const q = useQuery({
    queryKey: queryKeys.worklogs.range(fromUnix, toUnix),
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = q.data ?? [];
  // Overlap-based fetch (variant B): clip each worklog to the month window so a
  // worklog straddling the month boundary doesn't over-count.
  const monthSeconds = rows.reduce(
    (a, r) =>
      a +
      dayOverlapSeconds(
        r.started_at,
        r.ended_at ?? r.started_at + r.duration_s,
        fromUnix,
        toUnix + 1,
      ),
    0,
  );
  // Variant B (feedback #2): a worklog crossing local midnight counts only its
  // in-day slice toward today's total — matches the backend daily-goal check.
  const todayStartS = dayStartUnixS(today);
  const todayEndS = dayStartUnixS(addDays(today, 1));
  const todaySeconds = rows.reduce(
    (a, r) =>
      a +
      dayOverlapSeconds(
        r.started_at,
        r.ended_at ?? r.started_at + r.duration_s,
        todayStartS,
        todayEndS,
      ),
    0,
  );

  // Phase 18B — Item 3: prefer the user-configured working-week mask +
  // non-working-day calendar when counting working days. Falls back to the
  // legacy Mon–Fri assumption if the data hasn't loaded yet.
  const { mask, nonWorking } = useCalendarMask(monthStart, monthEnd);

  const workingDaysInMonth = useMemo(
    () => countWorkingDaysMasked(monthStart, monthEnd, mask, nonWorking),
    [monthStart, monthEnd, mask, nonWorking],
  );
  const workingDaysElapsed = useMemo(
    () => countWorkingDaysMasked(monthStart, today, mask, nonWorking),
    [monthStart, today, mask, nonWorking],
  );
  const remainingWorkingDays = Math.max(
    0,
    workingDaysInMonth - workingDaysElapsed,
  );
  // Strictly future working days (today is already counted in elapsed).
  const futureWorkingDays = useMemo(() => {
    const tomorrow = addDays(today, 1);
    if (tomorrow > monthEnd) return 0;
    return countWorkingDaysMasked(tomorrow, monthEnd, mask, nonWorking);
  }, [today, monthEnd, mask, nonWorking]);

  const monthGoalSeconds = workingDaysInMonth * dailyGoalSeconds;
  const expectedByTodaySeconds = workingDaysElapsed * dailyGoalSeconds;
  const paceDifferenceSeconds = monthSeconds - expectedByTodaySeconds;

  const todayPct = clampPct(todaySeconds / Math.max(1, dailyGoalSeconds));
  const monthPct = clampPct(monthSeconds / Math.max(1, monthGoalSeconds));

  return (
    <PageContainer gap="gap-4">
      <div className="flex items-start justify-between gap-4 flex-wrap pt-2">
        <div>
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Cíle
          </h1>
          <p className="text-xs text-[var(--text-tertiary)] mt-0.5">
            {MONTHS_LONG[today.getMonth()]} {today.getFullYear()} ·{" "}
            denní cíl {dailyGoalHours}h
          </p>
        </div>
        <div
          className="px-3 h-7 inline-flex items-center rounded-full
                     border border-[var(--border-subtle)]
                     text-[11px] text-[var(--text-tertiary)]"
        >
          {workingDaysElapsed} / {workingDaysInMonth} prac. dní uplynulo
        </div>
      </div>

      <ProgressCard
        title="Dnes"
        subtitle={`${WEEKDAYS_LONG[today.getDay()]}, ${today.getDate()}. ${
          MONTHS_LONG[today.getMonth()]
        }`}
        value={todaySeconds}
        goal={dailyGoalSeconds}
        valueLabel={`cíl: ${dailyGoalHours}h`}
        percent={todayPct}
      />

      <ProgressCard
        title="Tento měsíc"
        subtitle={`${workingDaysInMonth} pracovních dní · ${dailyGoalHours}h každý = celkem ${workingDaysInMonth * dailyGoalHours}h`}
        value={monthSeconds}
        goal={monthGoalSeconds}
        valueLabel={`cíl: ${workingDaysInMonth * dailyGoalHours}h`}
        percent={monthPct}
      />

      {/* Phase 18B — Item 3: month-end prediction card. */}
      <GoalsPrediction
        actualSeconds={monthSeconds}
        monthlyGoalSeconds={monthGoalSeconds}
        workingDaysElapsed={workingDaysElapsed}
        workingDaysRemaining={futureWorkingDays}
      />

      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <StatCard
          label="Očekáváno do dnes"
          value={formatDurationShort(expectedByTodaySeconds)}
        />
        <StatCard
          label="Skutečně zalogováno"
          value={formatDurationShort(monthSeconds)}
        />
        <StatCard
          label="Rozdíl tempa"
          value={formatSignedDuration(paceDifferenceSeconds)}
          tone={paceDifferenceSeconds >= 0 ? "neutral" : "danger"}
        />
        <StatCard
          label="Zbývající dny"
          value={`${Math.max(0, remainingWorkingDays)}`}
        />
      </div>
    </PageContainer>
  );
}

function ProgressCard({
  title,
  subtitle,
  value,
  goal,
  valueLabel,
  percent,
}: {
  title: string;
  subtitle: string;
  value: number;
  goal: number;
  valueLabel: string;
  percent: number;
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-5">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div>
          <h3 className="text-base font-semibold text-[var(--text-primary)]">
            {title}
          </h3>
          <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
            {subtitle}
          </p>
        </div>
        <div className="text-right">
          <div className="text-xl font-semibold text-[var(--accent)] tabular-nums">
            {formatDurationShort(value)}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)]">
            {valueLabel}
          </div>
        </div>
      </div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(percent)}
        className="h-2 rounded-full overflow-hidden"
        style={{ background: "var(--accent-soft)" }}
      >
        <div
          className="h-full transition-all duration-300 ease-out"
          style={{
            width: `${percent.toFixed(1)}%`,
            background: "var(--accent)",
          }}
        />
      </div>
      <div className="flex items-center justify-between mt-2 text-[11px]">
        <span className="text-[var(--text-tertiary)] tabular-nums">
          {formatDurationShort(value)} / {formatDurationShort(goal)}
        </span>
        <span className="text-[var(--text-tertiary)] tabular-nums">
          {Math.round(percent)}%
        </span>
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: string;
  tone?: "neutral" | "danger";
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] p-4">
      <div className="text-[11px] text-[var(--text-tertiary)]">{label}</div>
      <div
        className="mt-1 text-lg font-semibold tabular-nums"
        style={{
          color: tone === "danger" ? "var(--danger)" : "var(--text-primary)",
        }}
      >
        {value}
      </div>
    </div>
  );
}

/** Count Mon-Fri days in [from, to] inclusive. */
export function countWorkingDays(from: Date, to: Date): number {
  if (to < from) return 0;
  const d = new Date(from);
  d.setHours(0, 0, 0, 0);
  let n = 0;
  while (d <= to) {
    const dow = d.getDay();
    if (dow !== 0 && dow !== 6) n++;
    d.setDate(d.getDate() + 1);
  }
  return n;
}

/**
 * Phase 18B — Item 3: count working days in `[from, to]` inclusive using the
 * user's working-week mask + non-working-day calendar. Falls back to Mon–Fri
 * when the mask is unavailable.
 */
function countWorkingDaysMasked(
  from: Date,
  to: Date,
  mask: number,
  nonWorking: Set<string>,
): number {
  if (to < from) return 0;
  const d = new Date(from);
  d.setHours(0, 0, 0, 0);
  let n = 0;
  while (d <= to) {
    if (isWorkingDayLocal(d, mask, nonWorking)) n++;
    d.setDate(d.getDate() + 1);
  }
  return n;
}

function clampPct(ratio: number): number {
  if (!Number.isFinite(ratio) || ratio <= 0) return 0;
  return Math.min(100, ratio * 100);
}

function formatSignedDuration(seconds: number): string {
  if (seconds === 0) return "0m";
  const sign = seconds < 0 ? "−" : "+";
  return `${sign}${formatDurationShort(Math.abs(seconds))}`;
}

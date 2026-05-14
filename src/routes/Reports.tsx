/**
 * Reports route — aggregate insights over a configurable date range.
 *
 * Sections:
 *   1. Range picker + Export CSV.
 *   2. Summary cards (Earnings + total + averages).
 *   3. Daily bar chart.
 *   4. Per-project breakdown table + donut chart.
 *   5. Top issues table.
 */
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { getWorklogsForRange } from "../api/commands";
import { Card } from "../components/common/Card";
import { Spinner } from "../components/common/Spinner";
import { DailyBarChart } from "../components/Reports/DailyBarChart";
import { ExportButton } from "../components/Reports/ExportButton";
import { ProjectBreakdownTable } from "../components/Reports/ProjectBreakdownTable";
import { ProjectDonutChart } from "../components/Reports/ProjectDonutChart";
import { RangePicker } from "../components/Reports/RangePicker";
import { SummaryCards } from "../components/Reports/SummaryCards";
import { TopIssuesTable } from "../components/Reports/TopIssuesTable";
import { useDateRange } from "../hooks/useDateRange";
import {
  dayEndUnixS,
  dayStartUnixS,
  daysBetween,
  startOfDay,
} from "../lib/dates";
import { usePrefsStore } from "../stores/prefsStore";

export default function Reports() {
  const range = useDateRange("last_7");
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);

  const fromUnix = useMemo(() => dayStartUnixS(range.from), [range.from]);
  const toUnix = useMemo(() => dayEndUnixS(range.to), [range.to]);

  const q = useQuery({
    queryKey: ["worklogs-range", fromUnix, toUnix],
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = q.data ?? [];
  const totalSeconds = rows.reduce((acc, r) => acc + r.duration_s, 0);

  const daysInRange = Math.max(
    1,
    daysBetween(startOfDay(range.from), startOfDay(range.to)) + 1,
  );
  const daysWorked = useMemo(() => {
    const set = new Set<string>();
    for (const r of rows) {
      const d = new Date(r.started_at * 1000);
      set.add(`${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`);
    }
    return set.size;
  }, [rows]);

  return (
    <div className="p-8 flex flex-col gap-5 max-w-6xl mx-auto w-full">
      <Card padding="md">
        <div className="flex items-center justify-between gap-3 flex-wrap">
          <RangePicker
            preset={range.preset}
            from={range.from}
            to={range.to}
            onPresetChange={range.setPreset}
            onFromChange={range.setFrom}
            onToChange={range.setTo}
          />
          <ExportButton
            rows={rows}
            from={range.from}
            to={range.to}
            hourlyRate={hourlyRate}
            currency={currency}
          />
        </div>
      </Card>

      {q.isLoading && (
        <div className="flex items-center justify-center py-8 text-[var(--text-tertiary)] gap-2">
          <Spinner className="w-4 h-4" />
          <span className="text-xs">Crunching worklogs…</span>
        </div>
      )}

      <SummaryCards
        totalSeconds={totalSeconds}
        daysInRange={daysInRange}
        daysWorked={daysWorked}
        hourlyRate={hourlyRate}
        currency={currency}
      />

      {hourlyRate === 0 && (
        <Card padding="md" className="!bg-transparent !border-dashed">
          <p className="text-xs text-[var(--text-tertiary)]">
            Set an hourly rate in{" "}
            <span className="text-[var(--text-secondary)]">Settings → Time</span>{" "}
            to see earnings broken down by project here.
          </p>
        </Card>
      )}

      <Card padding="md" header={<span className="font-semibold">Daily totals</span>}>
        <DailyBarChart from={range.from} to={range.to} rows={rows} />
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card padding="md" header={<span className="font-semibold">By project</span>}>
          <div className="flex flex-col gap-4">
            <ProjectDonutChart rows={rows} />
            <ProjectBreakdownTable
              rows={rows}
              hourlyRate={hourlyRate}
              currency={currency}
            />
          </div>
        </Card>
        <Card padding="md" header={<span className="font-semibold">Top issues</span>}>
          <TopIssuesTable rows={rows} />
        </Card>
      </div>
    </div>
  );
}

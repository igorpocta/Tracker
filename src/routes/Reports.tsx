/**
 * Reports route — aggregate insights over a configurable date range.
 *
 * Sections:
 *   1. Range picker + Export CSV.
 *   2. Summary cards (total hours, avg/day, days worked).
 *   3. Daily bar chart.
 *   4. Per-project donut chart.
 *   5. Top issues table.
 */
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { getWorklogsForRange } from "../api/commands";
import { Card } from "../components/common/Card";
import { Spinner } from "../components/common/Spinner";
import { DailyBarChart } from "../components/Reports/DailyBarChart";
import { ExportButton } from "../components/Reports/ExportButton";
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

export default function Reports() {
  const range = useDateRange("last_7");

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
    <div className="p-6 flex flex-col gap-4 max-w-6xl mx-auto w-full">
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
          <ExportButton rows={rows} from={range.from} to={range.to} />
        </div>
      </Card>

      {q.isLoading && (
        <div className="flex items-center justify-center py-8 text-neutral-500 gap-2">
          <Spinner className="w-4 h-4" />
          <span className="text-xs">Crunching worklogs…</span>
        </div>
      )}

      <SummaryCards
        totalSeconds={totalSeconds}
        daysInRange={daysInRange}
        daysWorked={daysWorked}
      />

      <Card padding="md" header={<span className="font-semibold">Daily totals</span>}>
        <DailyBarChart from={range.from} to={range.to} rows={rows} />
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card padding="md" header={<span className="font-semibold">By project</span>}>
          <ProjectDonutChart rows={rows} />
        </Card>
        <Card padding="md" header={<span className="font-semibold">Top issues</span>}>
          <TopIssuesTable rows={rows} />
        </Card>
      </div>
    </div>
  );
}

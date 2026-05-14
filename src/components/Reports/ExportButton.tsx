/**
 * CSV export button for the Reports route.
 *
 * Includes an `earnings_<currency>` column when an hourly rate is set —
 * computed at the row level so per-issue costs can be reconciled offline.
 */
import { Download } from "lucide-react";

import type { WorklogRow } from "../../api/types";
import { buildCsv, downloadCsv } from "../../lib/csv";
import { formatIsoDate } from "../../lib/dates";
import { Button } from "../common/Button";

export interface ExportButtonProps {
  rows: WorklogRow[];
  from: Date;
  to: Date;
  hourlyRate: number;
  currency: string;
}

export function ExportButton({
  rows,
  from,
  to,
  hourlyRate,
  currency,
}: ExportButtonProps) {
  const handle = () => {
    const filename = `tracker-report-${formatIsoDate(from)}-to-${formatIsoDate(to)}.csv`;
    const includeEarnings = hourlyRate > 0;
    const header = [
      "issue_key",
      "summary",
      "started_at_iso",
      "duration_seconds",
      "duration_minutes",
      "duration_hours",
      ...(includeEarnings ? [`earnings_${currency.toLowerCase()}`] : []),
      "comment",
      "jira_worklog_id",
    ];
    const body = rows.map((r) => {
      const startedIso = new Date(r.started_at * 1000).toISOString();
      const seconds = r.duration_s;
      const minutes = Math.round(seconds / 60);
      const hours = Math.round((seconds / 3600) * 100) / 100;
      const earnings =
        Math.round((seconds / 3600) * hourlyRate * 100) / 100;
      const base: (string | number)[] = [
        r.issue_key,
        r.summary ?? "",
        startedIso,
        seconds,
        minutes,
        hours,
      ];
      if (includeEarnings) base.push(earnings);
      base.push(r.comment ?? "", r.jira_worklog_id ?? "");
      return base;
    });
    const csv = buildCsv(header, body);
    downloadCsv(filename, csv);
  };
  return (
    <Button
      variant="secondary"
      size="sm"
      onClick={handle}
      disabled={rows.length === 0}
    >
      <Download className="w-3.5 h-3.5" aria-hidden />
      Export CSV
    </Button>
  );
}

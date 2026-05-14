/**
 * CSV export button for the Reports route.
 *
 * Builds the CSV in memory from the in-range worklog rows and triggers a
 * Blob download (`URL.createObjectURL`) — no Tauri save dialog needed.
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
}

export function ExportButton({ rows, from, to }: ExportButtonProps) {
  const handle = () => {
    const filename = `tracker-report-${formatIsoDate(from)}-to-${formatIsoDate(to)}.csv`;
    const header = [
      "issue_key",
      "summary",
      "started_at_iso",
      "duration_seconds",
      "duration_minutes",
      "duration_hours",
      "comment",
      "jira_worklog_id",
    ];
    const body = rows.map((r) => {
      const startedIso = new Date(r.started_at * 1000).toISOString();
      const seconds = r.duration_s;
      const minutes = Math.round(seconds / 60);
      const hours = Math.round((seconds / 3600) * 100) / 100;
      return [
        r.issue_key,
        r.summary ?? "",
        startedIso,
        seconds,
        minutes,
        hours,
        r.comment ?? "",
        r.jira_worklog_id ?? "",
      ];
    });
    const csv = buildCsv(header, body);
    downloadCsv(filename, csv);
  };
  return (
    <Button variant="secondary" size="sm" onClick={handle} disabled={rows.length === 0}>
      <Download className="w-3.5 h-3.5" aria-hidden />
      Export CSV
    </Button>
  );
}

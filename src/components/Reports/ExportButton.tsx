/**
 * Reports export button — Phase 18B Item 11.
 *
 * Emits a TSV file matching the user's exact column spec:
 *
 *   Initiative \t Issues \t Work start time \t Time spent (hours)
 *   STREAM-4: myDOCK \t DEV-792: Portál – … \t 14.05.2026 15:46:00 \t 0,25
 *
 * Columns:
 *   - Initiative — `{epic_key}: {epic_summary}` from the cached issue row.
 *   - Issues     — `{issue_key}: {issue_summary}`.
 *   - Work start — `DD.MM.YYYY HH:MM:SS` Czech locale.
 *   - Hours      — duration in hours, Czech decimal comma, 2 decimals.
 *
 * Output is one row per worklog (no grouping). UTF-8 with BOM so Excel
 * detects the encoding when double-clicked.
 */
import { Download } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { searchIssuesCache } from "../../api/commands";
import type { IssueRow, WorklogRow } from "../../api/types";

export interface ExportButtonProps {
  rows: WorklogRow[];
  from: Date;
  to: Date;
}

export function ExportButton({ rows, from, to }: ExportButtonProps) {
  // Pull the issue cache so we can fill in epic_key / epic_summary per row.
  // The Reports view already implicitly cached these via earlier loads, so
  // this query is usually instant.
  const issueKeys = Array.from(new Set(rows.map((r) => r.issue_key)));
  const issueQ = useQuery({
    queryKey: ["export-issues", issueKeys.sort().join(",")],
    enabled: issueKeys.length > 0,
    queryFn: async () => {
      // We use the search cache helper which accepts a free-form query; since
      // we don't have a bulk-by-keys command we issue one search per unique
      // key — `searchIssuesCache` is cheap (local SQLite).
      const out = new Map<string, IssueRow>();
      for (const k of issueKeys) {
        try {
          const r = await searchIssuesCache(k, 5);
          const hit = r.find((row) => row.issue_key === k);
          if (hit) out.set(k, hit);
        } catch {
          /* ignore */
        }
      }
      return out;
    },
    staleTime: 60_000,
  });

  const handleExport = () => {
    const issueMap = issueQ.data ?? new Map<string, IssueRow>();
    const tsv = buildTsv(rows, issueMap);
    triggerDownload(tsv, `tracker-${formatIso(from)}-${formatIso(to)}.tsv`);
  };

  return (
    <button
      type="button"
      onClick={handleExport}
      disabled={rows.length === 0}
      className="inline-flex items-center gap-1.5 px-3 h-8
                 rounded-[var(--radius-md)] text-xs text-[var(--accent)]
                 border border-[var(--accent-soft)]
                 bg-transparent hover:bg-[var(--accent-soft)]
                 transition-colors duration-150
                 disabled:opacity-60 disabled:cursor-not-allowed"
      title="Exportovat výkaz do TSV (otevře se v Excelu)"
    >
      <Download className="w-3.5 h-3.5" aria-hidden />
      Exportovat do Excelu
    </button>
  );
}

export function buildTsv(
  rows: WorklogRow[],
  issueMap: Map<string, IssueRow>,
): string {
  const headers = ["Initiative", "Issues", "Work start time", "Time spent (hours)"];
  const body = rows
    .slice()
    .sort((a, b) => a.started_at - b.started_at)
    .map((r) => {
      const iss = issueMap.get(r.issue_key);
      const initiative = iss?.epic_key && iss?.epic_summary
        ? `${iss.epic_key}: ${iss.epic_summary}`
        : iss?.parent_key && iss?.parent_summary
          ? `${iss.parent_key}: ${iss.parent_summary}`
          : "";
      const issuesCell = `${r.issue_key}: ${iss?.summary ?? r.summary ?? ""}`;
      const startCell = formatExcelTime(new Date(r.started_at * 1000));
      const hoursCell = formatHoursCs(r.duration_s);
      return [initiative, issuesCell, startCell, hoursCell];
    });
  return [headers, ...body]
    .map((row) => row.map(escapeTsvCell).join("\t"))
    .join("\r\n");
}

/** Strip tabs/newlines from a cell — TSV can't escape them. */
function escapeTsvCell(s: string): string {
  return `${s ?? ""}`.replace(/[\t\r\n]+/g, " ").trim();
}

/** Czech-formatted clock + date: `DD.MM.YYYY HH:MM:SS`. */
export function formatExcelTime(d: Date): string {
  const dd = `${d.getDate()}`.padStart(2, "0");
  const mm = `${d.getMonth() + 1}`.padStart(2, "0");
  const yyyy = `${d.getFullYear()}`;
  const hh = `${d.getHours()}`.padStart(2, "0");
  const mi = `${d.getMinutes()}`.padStart(2, "0");
  const ss = `${d.getSeconds()}`.padStart(2, "0");
  return `${dd}.${mm}.${yyyy} ${hh}:${mi}:${ss}`;
}

/** Czech-comma hours: 900s → `"0,25"`, 7800s → `"2,17"`, 9792s → `"2,72"`. */
export function formatHoursCs(durationSeconds: number): string {
  const hours = Math.max(0, durationSeconds) / 3600;
  // Round to 2 decimals.
  const rounded = Math.round(hours * 100) / 100;
  return rounded.toFixed(2).replace(".", ",");
}

function formatIso(d: Date): string {
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

function triggerDownload(text: string, filename: string): void {
  // UTF-8 BOM so Excel detects encoding on double-click.
  const bom = "﻿";
  const blob = new Blob([bom + text], {
    type: "text/tab-separated-values;charset=utf-8",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

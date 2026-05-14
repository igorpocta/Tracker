/**
 * Compact worklog row showing the clock-time range, issue key + summary,
 * comment snippet, duration, and a "synced to Jira" badge.
 *
 * Used by Today, History, and (subset of) Reports. The `density` body class
 * adjusts vertical padding so the same component reads well in both modes.
 */
import { clsx } from "clsx";
import { CheckCircle2, CircleSlash } from "lucide-react";

import type { WorklogRow as Worklog } from "../../api/types";
import {
  formatClockTime,
  formatDurationShort,
} from "../../lib/format";

export interface WorklogRowProps {
  row: Worklog;
  /** Highlight rows where the timer was still running when the row was logged. */
  highlight?: boolean;
}

export function WorklogRow({ row, highlight = false }: WorklogRowProps) {
  const synced = !!row.jira_worklog_id;
  const startedMs = row.started_at * 1000;
  const endedMs = startedMs + row.duration_s * 1000;

  return (
    <li
      className={clsx(
        "worklog-row group rounded-md px-3 border border-transparent flex items-start gap-3",
        highlight
          ? "bg-emerald-600/5 border-emerald-700/30"
          : "hover:bg-neutral-800/50 hover:border-neutral-800",
      )}
    >
      <div
        className="mt-1 shrink-0"
        title={synced ? "Synced to Jira" : "Saved locally, not synced"}
      >
        {synced ? (
          <CheckCircle2
            className="w-3.5 h-3.5 text-emerald-400"
            aria-label="Synced to Jira"
          />
        ) : (
          <CircleSlash
            className="w-3.5 h-3.5 text-amber-400"
            aria-label="Local only"
          />
        )}
      </div>

      <div className="shrink-0 font-mono tabular-nums text-[11px] text-neutral-400 w-[88px] mt-0.5">
        {formatClockTime(startedMs)}–{formatClockTime(endedMs)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 min-w-0">
          <span className="font-mono text-[11px] text-neutral-400 shrink-0">
            {row.issue_key}
          </span>
          {row.summary && (
            <span className="text-xs text-neutral-200 truncate">
              {row.summary}
            </span>
          )}
        </div>
        {row.comment && (
          <p className="text-xs text-neutral-400 mt-0.5 line-clamp-2">
            {row.comment}
          </p>
        )}
      </div>

      <div className="text-right shrink-0">
        <div className="font-mono text-xs text-neutral-100">
          {formatDurationShort(row.duration_s)}
        </div>
      </div>
    </li>
  );
}

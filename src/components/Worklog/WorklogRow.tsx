/**
 * Compact worklog row: clock-time range, issue key + summary, comment snippet,
 * duration, and a small sync dot.
 *
 * No bright statuses — the sync dot is a subtle neutral mark; an accent-tinted
 * dot only flags rows that are pending local sync (no jira_worklog_id).
 */
import { clsx } from "clsx";

import type { WorklogRow as Worklog } from "../../api/types";
import {
  formatClockTime,
  formatDurationShort,
} from "../../lib/format";

export interface WorklogRowProps {
  row: Worklog;
  /** Highlight rows where the timer is still running for this issue. */
  highlight?: boolean;
}

export function WorklogRow({ row, highlight = false }: WorklogRowProps) {
  const synced = !!row.jira_worklog_id;
  const startedMs = row.started_at * 1000;
  const endedMs = startedMs + row.duration_s * 1000;

  return (
    <li
      className={clsx(
        "worklog-row group rounded-[var(--radius-sm)] px-3 border border-transparent flex items-start gap-3 transition-colors duration-150",
        highlight
          ? "bg-[var(--accent-soft)]"
          : "hover:bg-[var(--bg-hover)]",
      )}
    >
      <div
        className="mt-2 shrink-0"
        title={synced ? "Synced to Jira" : "Saved locally, not synced"}
        aria-label={synced ? "Synced to Jira" : "Local only"}
      >
        <span
          aria-hidden
          className={clsx(
            "block w-1.5 h-1.5 rounded-full",
            synced ? "bg-[var(--text-disabled)]" : "bg-[var(--accent)]",
          )}
        />
      </div>

      <div className="shrink-0 font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] w-[88px] mt-0.5">
        {formatClockTime(startedMs)}–{formatClockTime(endedMs)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 min-w-0">
          <span className="font-mono text-[11px] uppercase text-[var(--text-secondary)] shrink-0">
            {row.issue_key}
          </span>
          {row.summary && (
            <span className="text-xs text-[var(--text-primary)] truncate">
              {row.summary}
            </span>
          )}
        </div>
        {row.comment && (
          <p className="text-xs text-[var(--text-tertiary)] mt-0.5 line-clamp-2">
            {row.comment}
          </p>
        )}
      </div>

      <div className="text-right shrink-0">
        <div className="font-mono tabular-nums text-xs text-[var(--text-primary)]">
          {formatDurationShort(row.duration_s)}
        </div>
      </div>
    </li>
  );
}

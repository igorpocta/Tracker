/**
 * A single row in the worklog history list. Shows issue key, summary,
 * duration, comment (if any), and a relative "logged at" timestamp.
 */
import { CheckCircle2, CircleSlash } from "lucide-react";

import type { WorklogRow as Worklog } from "../../api/types";
import {
  formatDurationShort,
  formatRelativeTime,
} from "../../lib/format";

export interface WorklogRowProps {
  row: Worklog;
  /** Reference now used for the relative timestamp. */
  now: Date;
}

export function WorklogRow({ row, now }: WorklogRowProps) {
  const synced = !!row.jira_worklog_id;
  return (
    <li className="px-3 py-2 rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)] flex items-start gap-3 transition-colors duration-150">
      <div
        className="mt-0.5"
        title={synced ? "Synced to Jira" : "Saved locally, not synced"}
      >
        {synced ? (
          <CheckCircle2
            className="w-3.5 h-3.5 text-[var(--text-disabled)]"
            aria-label="Synced to Jira"
          />
        ) : (
          <CircleSlash
            className="w-3.5 h-3.5 text-[var(--accent)]"
            aria-label="Local only"
          />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] uppercase text-[var(--text-secondary)]">
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
        <div className="text-[10px] text-[var(--text-tertiary)]">
          {formatRelativeTime(row.logged_at, now)}
        </div>
      </div>
    </li>
  );
}

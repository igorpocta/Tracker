/**
 * Generic list of worklog rows, sharing a presentation between Today and
 * History (and Reports' "top issues" hover row in the future).
 *
 * The list is ordered by `started_at` ascending so the day reads top→bottom
 * chronologically.
 */
import type { WorklogRow as Worklog } from "../../api/types";
import { Spinner } from "../common/Spinner";
import { EmptyState } from "./EmptyState";
import { WorklogRow } from "./WorklogRow";

export interface WorklogListProps {
  rows: Worklog[] | undefined;
  loading?: boolean;
  emptyTitle?: string;
  emptyDescription?: string;
  /** Issue key of the currently running timer (for visual highlight). */
  activeIssueKey?: string | null;
}

export function WorklogList({
  rows,
  loading = false,
  emptyTitle = "No worklogs yet",
  emptyDescription,
  activeIssueKey,
}: WorklogListProps) {
  if (loading && (!rows || rows.length === 0)) {
    return (
      <div className="flex items-center justify-center py-8 text-[var(--text-tertiary)] gap-2">
        <Spinner className="w-4 h-4" />
        <span className="text-xs">Loading worklogs…</span>
      </div>
    );
  }

  if (!rows || rows.length === 0) {
    return (
      <EmptyState title={emptyTitle} description={emptyDescription} />
    );
  }

  // Stable sort by start time ascending — natural reading order for a day.
  const ordered = [...rows].sort((a, b) => a.started_at - b.started_at);

  return (
    <ul className="flex flex-col">
      {ordered.map((row) => (
        <WorklogRow
          key={row.id ?? `${row.issue_key}-${row.started_at}`}
          row={row}
          highlight={activeIssueKey === row.issue_key}
        />
      ))}
    </ul>
  );
}

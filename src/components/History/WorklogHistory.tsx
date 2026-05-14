/**
 * Worklog history list. Pulls recent rows via TanStack Query so the cache
 * survives navigations and refreshes on demand via the `worklog-saved`
 * Tauri event (wired up in `Home`).
 */
import { useQuery } from "@tanstack/react-query";

import { getWorklogIssues } from "../../api/commands";
import { useNow } from "../../hooks/useNow";
import { Spinner } from "../common/Spinner";
import { WorklogRow } from "./WorklogRow";

export interface WorklogHistoryProps {
  limit?: number;
}

export function WorklogHistory({ limit = 50 }: WorklogHistoryProps) {
  const query = useQuery({
    queryKey: ["worklog-history", limit],
    queryFn: () => getWorklogIssues(limit),
  });
  // Refresh relative timestamps once a minute.
  const nowMs = useNow(60_000);
  const now = new Date(nowMs);

  if (query.isLoading) {
    return (
      <div className="flex items-center justify-center py-6 text-[var(--text-tertiary)]">
        <Spinner className="w-4 h-4 mr-2" />
        Loading worklog history…
      </div>
    );
  }

  const rows = query.data ?? [];
  if (rows.length === 0) {
    return (
      <div className="text-sm text-[var(--text-tertiary)] py-8 text-center">
        No worklogs yet. Start a timer to log your first one.
      </div>
    );
  }

  return (
    <ul className="flex flex-col gap-0.5">
      {rows.map((row) => (
        <WorklogRow
          key={row.id ?? `${row.issue_key}-${row.logged_at}`}
          row={row}
          now={now}
        />
      ))}
    </ul>
  );
}

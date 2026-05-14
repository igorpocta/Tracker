/**
 * Recent-issues section in the sidebar. Calls `get_recent_issues` through
 * TanStack Query so the cache backs us between re-renders.
 */
import { useQuery } from "@tanstack/react-query";
import { Clock } from "lucide-react";

import { getRecentIssues } from "../../api/commands";
import { IssueList } from "./IssueList";

export interface RecentIssuesProps {
  selectedKey?: string | null;
  activeKey?: string | null;
  onSelect: (issueKey: string) => void;
  limit?: number;
}

export function RecentIssues({
  selectedKey,
  activeKey,
  onSelect,
  limit = 15,
}: RecentIssuesProps) {
  const query = useQuery({
    queryKey: ["recent-issues", limit],
    queryFn: () => getRecentIssues(limit),
  });

  return (
    <IssueList
      title="Recent"
      icon={<Clock className="w-3 h-3" aria-hidden />}
      issues={query.data ?? []}
      loading={query.isLoading}
      selectedKey={selectedKey}
      activeKey={activeKey}
      onSelect={onSelect}
      emptyMessage="No recently updated issues. Click Refresh to sync."
    />
  );
}

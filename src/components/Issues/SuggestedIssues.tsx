/**
 * Suggested issues = ones with prior worklogs, ordered by last log time.
 * Effectively a "you usually track these" picker.
 */
import { useQuery } from "@tanstack/react-query";
import { Star } from "lucide-react";

import { getSuggestedIssues } from "../../api/commands";
import { IssueList } from "./IssueList";

export interface SuggestedIssuesProps {
  selectedKey?: string | null;
  activeKey?: string | null;
  onSelect: (issueKey: string) => void;
  limit?: number;
}

export function SuggestedIssues({
  selectedKey,
  activeKey,
  onSelect,
  limit = 10,
}: SuggestedIssuesProps) {
  const query = useQuery({
    queryKey: ["suggested-issues", limit],
    queryFn: () => getSuggestedIssues(limit),
  });

  return (
    <IssueList
      title="Suggested"
      icon={<Star className="w-3 h-3" aria-hidden />}
      issues={query.data ?? []}
      loading={query.isLoading}
      selectedKey={selectedKey}
      activeKey={activeKey}
      onSelect={onSelect}
      emptyMessage="Track some time and we'll suggest issues here."
    />
  );
}

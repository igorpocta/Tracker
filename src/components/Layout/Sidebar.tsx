/**
 * Left sidebar: search input on top, then search-results when the user has
 * typed something, otherwise the Suggested + Recent lists.
 *
 * Search results are debounced (~150ms) so we don't fire an IPC roundtrip
 * on every keystroke.
 */
import { useQuery } from "@tanstack/react-query";
import { Search } from "lucide-react";
import { useEffect, useState } from "react";

import { searchIssuesCache } from "../../api/commands";
import { IssueList } from "../Issues/IssueList";
import { RecentIssues } from "../Issues/RecentIssues";
import { SearchInput } from "../Issues/SearchInput";
import { SuggestedIssues } from "../Issues/SuggestedIssues";

export interface SidebarProps {
  selectedKey?: string | null;
  activeKey?: string | null;
  onSelect: (issueKey: string) => void;
}

export function Sidebar({ selectedKey, activeKey, onSelect }: SidebarProps) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");

  // 150ms debounce — feels live without hammering SQLite for every keystroke.
  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), 150);
    return () => window.clearTimeout(t);
  }, [query]);

  const searchQuery = useQuery({
    queryKey: ["search-issues", debounced],
    queryFn: () => searchIssuesCache(debounced, 30),
    enabled: debounced.length > 0,
  });

  const isSearching = debounced.length > 0;

  return (
    <aside className="w-64 shrink-0 border-r border-neutral-800/70 bg-neutral-950/30 flex flex-col">
      <div className="p-3 border-b border-neutral-800/70">
        <SearchInput value={query} onChange={setQuery} />
      </div>

      <div className="flex-1 overflow-y-auto p-2 flex flex-col gap-4">
        {isSearching ? (
          <IssueList
            title="Search results"
            icon={<Search className="w-3 h-3" aria-hidden />}
            issues={searchQuery.data ?? []}
            loading={searchQuery.isLoading || searchQuery.isFetching}
            selectedKey={selectedKey}
            activeKey={activeKey}
            onSelect={onSelect}
            emptyMessage={`No matches for "${debounced}".`}
          />
        ) : (
          <>
            <SuggestedIssues
              selectedKey={selectedKey}
              activeKey={activeKey}
              onSelect={onSelect}
            />
            <RecentIssues
              selectedKey={selectedKey}
              activeKey={activeKey}
              onSelect={onSelect}
            />
          </>
        )}
      </div>
    </aside>
  );
}

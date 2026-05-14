/**
 * Generic sidebar issue list. Renders a header with optional count badge,
 * and either an empty state, a spinner, or the list of `IssueRow`s.
 */
import { clsx } from "clsx";
import type { ReactNode } from "react";

import type { IssueRow as Issue } from "../../api/types";
import { Spinner } from "../common/Spinner";
import { IssueRow } from "./IssueRow";

export interface IssueListProps {
  title: string;
  /** Optional icon shown next to the title. */
  icon?: ReactNode;
  issues: Issue[];
  loading?: boolean;
  /** Currently selected issue key (highlighted in the list). */
  selectedKey?: string | null;
  /** Issue key of the running timer, for the small dot indicator. */
  activeKey?: string | null;
  onSelect: (issueKey: string) => void;
  /** Message shown when the list is empty. */
  emptyMessage?: string;
  className?: string;
}

export function IssueList({
  title,
  icon,
  issues,
  loading = false,
  selectedKey,
  activeKey,
  onSelect,
  emptyMessage = "Nothing here yet",
  className,
}: IssueListProps) {
  return (
    <section className={clsx("flex flex-col gap-1", className)}>
      <header className="flex items-center gap-1.5 px-2 mb-1">
        {icon && <span className="text-[var(--text-tertiary)]">{icon}</span>}
        <h2 className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {title}
        </h2>
        {!loading && issues.length > 0 && (
          <span className="text-[10px] text-[var(--text-tertiary)] ml-auto font-mono tabular-nums">
            {issues.length}
          </span>
        )}
        {loading && <Spinner className="w-3 h-3 ml-auto text-[var(--text-tertiary)]" />}
      </header>

      {loading && issues.length === 0 ? (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">Loading…</div>
      ) : issues.length === 0 ? (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">{emptyMessage}</div>
      ) : (
        <ul className="flex flex-col gap-0.5">
          {issues.map((i) => (
            <li key={i.issue_key}>
              <IssueRow
                issue={i}
                selected={selectedKey === i.issue_key}
                active={activeKey === i.issue_key}
                onSelect={onSelect}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

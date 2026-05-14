/**
 * A single clickable row in the sidebar issue lists.
 *
 * Visual states:
 * - default: subtle hover.
 * - selected: accent-soft background.
 * - active timer: accent dot next to the key.
 */
import { clsx } from "clsx";

import type { IssueRow as Issue } from "../../api/types";

export interface IssueRowProps {
  issue: Issue;
  selected?: boolean;
  /** True if this issue is the active timer's subject. */
  active?: boolean;
  onSelect: (issueKey: string) => void;
}

export function IssueRow({
  issue,
  selected = false,
  active = false,
  onSelect,
}: IssueRowProps) {
  return (
    <button
      type="button"
      onClick={() => onSelect(issue.issue_key)}
      aria-current={selected ? "true" : undefined}
      className={clsx(
        "w-full text-left rounded-[var(--radius-sm)] px-2.5 py-1.5 transition-colors duration-150 flex items-start gap-2",
        selected
          ? "bg-[var(--accent-soft)] text-[var(--text-primary)]"
          : "hover:bg-[var(--bg-hover)] text-[var(--text-primary)]",
      )}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-[11px] uppercase text-[var(--text-secondary)] shrink-0">
            {issue.issue_key}
          </span>
          {active && (
            <span
              aria-label="Active timer"
              className="w-1.5 h-1.5 rounded-full bg-[var(--accent)] inline-block"
            />
          )}
        </div>
        <div className="text-xs truncate text-[var(--text-primary)]">
          {issue.summary || "(no summary)"}
        </div>
      </div>
    </button>
  );
}

/**
 * A single clickable row in the sidebar issue lists.
 *
 * Visual states:
 * - default: muted background, white text.
 * - selected: highlighted background (the right panel shows this issue).
 * - active timer: a small accent dot next to the key.
 */
import { clsx } from "clsx";
import { Dot } from "lucide-react";

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
        "w-full text-left rounded-md px-2.5 py-1.5 transition-colors flex items-start gap-1.5",
        selected
          ? "bg-sky-600/15 text-white ring-1 ring-sky-500/30"
          : "hover:bg-neutral-800/70 text-neutral-200",
      )}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="font-mono text-[11px] text-neutral-400 shrink-0">
            {issue.issue_key}
          </span>
          {active && (
            <Dot
              className="w-4 h-4 text-emerald-400 -ml-1"
              aria-label="Active timer"
            />
          )}
        </div>
        <div className="text-xs truncate text-neutral-100">
          {issue.summary || "(no summary)"}
        </div>
      </div>
    </button>
  );
}

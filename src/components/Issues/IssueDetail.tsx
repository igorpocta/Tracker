/**
 * Right-panel detail view for a selected issue.
 *
 * Shows the basic metadata we already have in the SQLite cache + two
 * actions: "Open in Jira" (launches the browser) and "Start timer".
 */
import { ExternalLink, Play, Square } from "lucide-react";

import { openJiraIssue } from "../../api/commands";
import type { IssueRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";
import { Button } from "../common/Button";

export interface IssueDetailProps {
  issue: IssueRow;
  /** True if the timer is running for this exact issue. */
  active?: boolean;
  /** Called when the user clicks "Start timer". */
  onStart: (issueKey: string) => void;
  /** Called when the user clicks "Stop" (only shown while active). */
  onStop?: () => void;
}

export function IssueDetail({
  issue,
  active = false,
  onStart,
  onStop,
}: IssueDetailProps) {
  return (
    <article className="flex flex-col gap-4">
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span className="font-mono text-xs text-neutral-400">
              {issue.issue_key}
            </span>
            {issue.status_category && (
              <StatusBadge category={issue.status_category} />
            )}
            {issue.issue_type && (
              <span className="text-[10px] uppercase tracking-wide text-neutral-500">
                {issue.issue_type}
              </span>
            )}
          </div>
          <h2 className="text-lg font-semibold leading-tight">
            {issue.summary || "(no summary)"}
          </h2>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              openJiraIssue(issue.issue_key).catch(() => {
                /* no toast yet — the browser command rarely fails. */
              });
            }}
            aria-label="Open in Jira"
          >
            <ExternalLink className="w-3.5 h-3.5" aria-hidden />
            Open in Jira
          </Button>
          {active ? (
            <Button variant="danger" size="sm" onClick={onStop}>
              <Square className="w-3.5 h-3.5" aria-hidden />
              Stop
            </Button>
          ) : (
            <Button
              variant="primary"
              size="sm"
              onClick={() => onStart(issue.issue_key)}
            >
              <Play className="w-3.5 h-3.5" aria-hidden />
              Start timer
            </Button>
          )}
        </div>
      </header>

      <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
        {issue.parent_key && (
          <Field label="Parent">
            <span className="font-mono text-neutral-300">
              {issue.parent_key}
            </span>
            {issue.parent_summary && (
              <span className="text-neutral-500 ml-1.5">
                — {issue.parent_summary}
              </span>
            )}
          </Field>
        )}
        {issue.epic_key && (
          <Field label="Epic">
            <span className="font-mono text-neutral-300">{issue.epic_key}</span>
            {issue.epic_summary && (
              <span className="text-neutral-500 ml-1.5">
                — {issue.epic_summary}
              </span>
            )}
          </Field>
        )}
        {issue.assignee_email && (
          <Field label="Assignee">{issue.assignee_email}</Field>
        )}
        {typeof issue.time_original_estimate === "number" &&
          issue.time_original_estimate > 0 && (
            <Field label="Estimate">
              {formatDurationShort(issue.time_original_estimate)}
            </Field>
          )}
        {typeof issue.time_spent === "number" && issue.time_spent > 0 && (
          <Field label="Logged">{formatDurationShort(issue.time_spent)}</Field>
        )}
        {typeof issue.time_estimate === "number" && issue.time_estimate > 0 && (
          <Field label="Remaining">
            {formatDurationShort(issue.time_estimate)}
          </Field>
        )}
      </dl>
    </article>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col">
      <dt className="text-[10px] uppercase tracking-wide text-neutral-500 mb-0.5">
        {label}
      </dt>
      <dd className="text-neutral-200">{children}</dd>
    </div>
  );
}

function StatusBadge({ category }: { category: string }) {
  const lower = category.toLowerCase();
  const tone =
    lower === "done"
      ? "bg-emerald-600/15 text-emerald-300"
      : lower === "indeterminate" || lower === "in progress"
        ? "bg-sky-600/15 text-sky-300"
        : "bg-neutral-700/40 text-neutral-300";
  return (
    <span
      className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide ${tone}`}
    >
      {category}
    </span>
  );
}

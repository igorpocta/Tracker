/**
 * Side-by-side before/after diff for an audit entry.
 *
 *   Před:  2h 15m · 15:46–18:01 · "Sync portál"
 *   Po:    — (smazáno)
 *
 * Renders three cells: duration, start–end clock, comment snippet. Cells
 * whose value differs between before and after get an accent-soft background
 * to draw the eye to what actually changed.
 *
 * For ops where one side is missing (e.g. `create` has no "Před", `delete`
 * has no "Po") the corresponding row is hidden entirely.
 */
import type { WorklogRow } from "../../api/types";
import {
  formatClockTime,
  formatDurationShort,
} from "../../lib/format";

export interface BeforeAfterDiffProps {
  before: WorklogRow | null;
  after: WorklogRow | null;
  /** When true, the missing-side row renders "— (smazáno)" instead of nothing. */
  showMissingAsDeleted?: boolean;
}

export function BeforeAfterDiff({
  before,
  after,
  showMissingAsDeleted = false,
}: BeforeAfterDiffProps) {
  const changed = computeChanged(before, after);

  return (
    <div className="grid grid-cols-[60px_1fr] gap-y-1 text-[12px] mt-2">
      {before && (
        <Row
          label="Před"
          row={before}
          changed={changed}
          variant="muted"
        />
      )}
      {after && (
        <Row label="Po" row={after} changed={changed} variant="primary" />
      )}
      {!after && before && showMissingAsDeleted && (
        <DeletedRow />
      )}
    </div>
  );
}

interface ChangedFlags {
  duration: boolean;
  start: boolean;
  comment: boolean;
  issueKey: boolean;
}

function computeChanged(
  before: WorklogRow | null,
  after: WorklogRow | null,
): ChangedFlags {
  if (!before || !after) {
    return { duration: false, start: false, comment: false, issueKey: false };
  }
  return {
    duration: before.duration_s !== after.duration_s,
    start: before.started_at !== after.started_at,
    comment: (before.comment ?? "") !== (after.comment ?? ""),
    issueKey: before.issue_key !== after.issue_key,
  };
}

function Row({
  label,
  row,
  changed,
  variant,
}: {
  label: "Před" | "Po";
  row: WorklogRow;
  changed: ChangedFlags;
  variant: "muted" | "primary";
}) {
  const textColor =
    variant === "muted" ? "var(--text-tertiary)" : "var(--text-primary)";
  const start = new Date(row.started_at * 1000);
  const end = new Date((row.started_at + row.duration_s) * 1000);
  return (
    <>
      <span
        className="text-[10px] uppercase tracking-[0.06em] pt-0.5"
        style={{ color: "var(--text-tertiary)" }}
      >
        {label}:
      </span>
      <div className="flex items-center gap-2 flex-wrap" style={{ color: textColor }}>
        <Cell highlight={changed.duration} className="font-mono tabular-nums">
          {formatDurationShort(row.duration_s)}
        </Cell>
        <span aria-hidden style={{ color: "var(--text-tertiary)" }}>
          ·
        </span>
        <Cell highlight={changed.start} className="font-mono tabular-nums">
          {formatClockTime(start)}–{formatClockTime(end)}
        </Cell>
        {changed.issueKey && (
          <>
            <span aria-hidden style={{ color: "var(--text-tertiary)" }}>·</span>
            <Cell highlight={true} className="font-mono">
              {row.issue_key}
            </Cell>
          </>
        )}
        {row.comment ? (
          <>
            <span aria-hidden style={{ color: "var(--text-tertiary)" }}>·</span>
            <Cell highlight={changed.comment} className="truncate max-w-[260px]">
              &ldquo;{truncate(row.comment, 60)}&rdquo;
            </Cell>
          </>
        ) : null}
      </div>
    </>
  );
}

function DeletedRow() {
  return (
    <>
      <span
        className="text-[10px] uppercase tracking-[0.06em] pt-0.5"
        style={{ color: "var(--text-tertiary)" }}
      >
        Po:
      </span>
      <span
        className="text-[12px]"
        style={{ color: "var(--text-tertiary)", fontStyle: "italic" }}
      >
        — (smazáno)
      </span>
    </>
  );
}

function Cell({
  highlight,
  className,
  children,
}: {
  highlight: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  if (!highlight) return <span className={className}>{children}</span>;
  return (
    <span
      className={`px-1 rounded-[var(--radius-sm)] ${className ?? ""}`}
      style={{
        background: "var(--accent-soft)",
        color: "var(--accent)",
      }}
    >
      {children}
    </span>
  );
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, max - 1)}…`;
}

/**
 * One audit entry as a card with op badge, before/after diff, and a
 * context-aware reconstruction button (right-aligned).
 *
 *   [Změněno] DEV-304 #34123                   14. 5. · 14:18
 *   Před: 1h 0m  · 10:00–11:00 · "Initial draft"
 *   Po:   1h 30m · 10:00–11:30 · "Initial draft + review"
 *   ✓ Úspěšně                                  [Vrátit změnu]
 */
import { Check, X } from "lucide-react";

import type { AuditEntry, WorklogRow } from "../../api/types";
import { useT, type TFunc } from "../../i18n";
import { ConfirmButton } from "../common/ConfirmButton";
import { formatClockTime } from "../../lib/format";

import { BeforeAfterDiff } from "./BeforeAfterDiff";
import { OpBadge } from "./OpBadge";

export interface AuditRowProps {
  entry: AuditEntry;
  /** True iff a newer audit entry has `source_audit_id == entry.id`. */
  alreadyReconstructed: boolean;
  busy: boolean;
  /** Restore: op = delete | sync_tombstone AND success. */
  onRestore?: (entry: AuditEntry) => Promise<void> | void;
  /** Revert: op = update AND success. */
  onRevert?: (entry: AuditEntry) => Promise<void> | void;
  /** Retry: any op AND success = false. */
  onRetry?: (entry: AuditEntry) => Promise<void> | void;
}

export function AuditRow({
  entry,
  alreadyReconstructed,
  busy,
  onRestore,
  onRevert,
  onRetry,
}: AuditRowProps) {
  const before = parseRow(entry.before_json);
  const after = parseRow(entry.after_json);

  return (
    <article
      className="rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] p-3 flex flex-col gap-1.5"
      data-testid={`audit-row-${entry.id}`}
    >
      {/* Header: op badge + issue key + clock */}
      <div className="flex items-baseline gap-2 flex-wrap">
        <OpBadge op={entry.op} />
        {entry.issue_key && (
          <span
            className="font-mono text-[11px] font-semibold"
            style={{ color: "var(--text-primary)" }}
          >
            {entry.issue_key}
          </span>
        )}
        {entry.worklog_id && (
          <span
            className="font-mono text-[10px]"
            style={{ color: "var(--text-tertiary)" }}
          >
            #{entry.worklog_id}
          </span>
        )}
        <div className="flex-1" />
        <span
          className="font-mono tabular-nums text-[10px]"
          style={{ color: "var(--text-tertiary)" }}
        >
          {formatClockTime(entry.occurred_at)}
        </span>
      </div>

      {/* Diff body */}
      <BeforeAfterDiff
        before={before}
        after={after}
        showMissingAsDeleted={entry.op === "delete" || entry.op === "sync_tombstone"}
      />

      {/* Status + action */}
      <div className="flex items-center justify-between gap-2 mt-1">
        <StatusIndicator success={entry.success} error={entry.error} />
        <ActionButtons
          entry={entry}
          alreadyReconstructed={alreadyReconstructed}
          busy={busy}
          onRestore={onRestore}
          onRevert={onRevert}
          onRetry={onRetry}
        />
      </div>
    </article>
  );
}

function StatusIndicator({
  success,
  error,
}: {
  success: boolean;
  error?: string | null;
}) {
  const t = useT();
  if (success) {
    return (
      <span
        className="inline-flex items-center gap-1 text-[11px]"
        style={{ color: "var(--success)" }}
      >
        <Check className="w-3 h-3" aria-hidden />
        {t("audit.status.success")}
      </span>
    );
  }
  return (
    <span
      className="inline-flex items-center gap-1 text-[11px]"
      style={{ color: "var(--danger)" }}
    >
      <X className="w-3 h-3" aria-hidden />
      {error
        ? t("audit.status.failedWithError", { error })
        : t("audit.status.failed")}
    </span>
  );
}

function ActionButtons({
  entry,
  alreadyReconstructed,
  busy,
  onRestore,
  onRevert,
  onRetry,
}: {
  entry: AuditEntry;
  alreadyReconstructed: boolean;
  busy: boolean;
  onRestore?: (e: AuditEntry) => Promise<void> | void;
  onRevert?: (e: AuditEntry) => Promise<void> | void;
  onRetry?: (e: AuditEntry) => Promise<void> | void;
}) {
  const t = useT();
  // Failed → retry button trumps everything.
  if (!entry.success) {
    return (
      <ConfirmButton
        label={t("audit.action.retry")}
        confirmLabel={t("audit.action.retryConfirm")}
        variant="secondary"
        disabled={busy}
        onConfirm={async () => {
          if (onRetry) await onRetry(entry);
        }}
      />
    );
  }
  if (alreadyReconstructed) {
    return (
      <span
        className="text-[10px] italic"
        style={{ color: "var(--text-tertiary)" }}
      >
        {t("audit.action.alreadyRestored")}
      </span>
    );
  }
  if (entry.op === "delete" || entry.op === "sync_tombstone") {
    return (
      <ConfirmButton
        label={t("audit.action.restore", {
          where: providerLocative(entry.issue_key, t),
        })}
        confirmLabel={t("audit.action.restoreConfirm")}
        variant="primary"
        disabled={busy}
        onConfirm={async () => {
          if (onRestore) await onRestore(entry);
        }}
      />
    );
  }
  if (entry.op === "update") {
    return (
      <ConfirmButton
        label={t("audit.action.revert")}
        confirmLabel={t("audit.action.revertConfirm")}
        variant="secondary"
        disabled={busy}
        onConfirm={async () => {
          if (onRevert) await onRevert(entry);
        }}
      />
    );
  }
  return null;
}

/**
 * Z prefixu issue klíče odhadne provider a vrátí lokativ pro tlačítko —
 * "v Jiře" / "ve Freelu". Pro neznámý prefix (nebo NULL klíč) padá na
 * obecné "v cloudu", ať tlačítko zůstane gramaticky čitelné.
 */
function providerLocative(issueKey: string | null | undefined, t: TFunc): string {
  if (!issueKey) return t("audit.provider.cloud");
  if (issueKey.startsWith("FREELO-")) return t("audit.provider.freelo");
  return t("audit.provider.jira");
}

function parseRow(s: string | null | undefined): WorklogRow | null {
  if (!s) return null;
  try {
    return JSON.parse(s) as WorklogRow;
  } catch {
    return null;
  }
}

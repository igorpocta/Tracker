/**
 * Nepřiřazené — review screen for worklogs that were logged without a task.
 *
 * A stopped-but-unassigned timer (or a manual entry with no issue) lands in
 * the DB with `issue_key = NULL`. Those rows are only visible on their own day
 * in the Time Log, so at month-end invoice prep they are easy to miss — and an
 * unbilled worklog is lost money. This screen gathers ALL of them in one place
 * (newest first) with an inline issue picker, plus a running total so you can
 * see how many hours are still un-invoiceable.
 *
 * The sidebar badge reads the same React Query (`worklogs.unassigned`), so the
 * count and this list stay in sync; assigning a row invalidates the `worklogs`
 * prefix and both refresh.
 */
import { useQuery } from "@tanstack/react-query";
import { CheckCircle2 } from "lucide-react";
import { useOutletContext } from "react-router-dom";

import { listUnassignedWorklogs } from "../api/commands";
import { queryKeys } from "../api/queryKeys";
import type { WorklogRow as ApiWorklogRow } from "../api/types";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { PageContainer } from "../components/Layout/PageContainer";
import { IssuePicker } from "../components/Worklog/IssuePicker";
import { useAssignWorklog } from "../hooks/useAssignWorklog";
import { formatDateCs, formatDurationShort, pluralCs } from "../lib/format";
import { formatHHMM } from "../lib/dates";
import { useT } from "../i18n";

export function Unassigned() {
  const t = useT();
  const ctx = useOutletContext<ShellOutletContext>();

  const rowsQ = useQuery({
    queryKey: queryKeys.worklogs.unassigned(),
    queryFn: listUnassignedWorklogs,
    staleTime: 10_000,
  });
  const rows = rowsQ.data ?? [];
  const totalSeconds = rows.reduce((a, r) => a + r.duration_s, 0);

  const handleAssign = useAssignWorklog(ctx.pushToast);

  return (
    <PageContainer>
      <div className="flex items-baseline justify-between gap-4 flex-wrap pt-2 mb-1">
        <h1 className="text-xl font-semibold text-[var(--text-primary)]">
          {t("unassigned.title")}
        </h1>
        {rows.length > 0 && (
          <div className="text-right">
            <div className="font-mono tabular-nums text-sm text-[var(--text-primary)]">
              {formatDurationShort(totalSeconds)}
            </div>
            <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
              {rows.length}{" "}
              {pluralCs(rows.length, [
                t("unassigned.count.one"),
                t("unassigned.count.few"),
                t("unassigned.count.many"),
              ])}{" "}
              {t("unassigned.toAssign")}
            </div>
          </div>
        )}
      </div>
      <p className="text-xs text-[var(--text-tertiary)] mb-4">
        {t("unassigned.description")}
      </p>

      {rowsQ.isLoading ? (
        <div className="text-sm text-[var(--text-tertiary)]">{t("unassigned.loading")}</div>
      ) : rows.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center gap-2">
          <CheckCircle2
            className="w-8 h-8 text-[var(--accent)]"
            aria-hidden
          />
          <div className="text-sm text-[var(--text-primary)]">
            {t("unassigned.allAssigned")}
          </div>
          <div className="text-xs text-[var(--text-tertiary)]">
            {t("unassigned.emptyHint")}
          </div>
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {rows.map((row) => (
            <UnassignedRow key={rowKey(row)} row={row} onAssign={handleAssign} />
          ))}
        </ul>
      )}
    </PageContainer>
  );
}

function UnassignedRow({
  row,
  onAssign,
}: {
  row: ApiWorklogRow;
  onAssign: (row: ApiWorklogRow, issueKey: string) => Promise<void>;
}) {
  const t = useT();
  const started = new Date(row.started_at * 1000);
  const ended = new Date((row.started_at + row.duration_s) * 1000);
  const comment = row.comment ?? row.description ?? null;

  return (
    <li
      className="flex items-center gap-3 h-12 px-3 rounded-[var(--radius-md)]
                 bg-[var(--bg-surface)] border border-[var(--border-subtle)]"
    >
      <span className="font-mono tabular-nums text-xs text-[var(--text-secondary)] w-28 shrink-0">
        {formatDateCs(started)}
      </span>
      <span className="font-mono tabular-nums text-xs text-[var(--text-tertiary)] w-28 shrink-0">
        {formatHHMM(started)} – {formatHHMM(ended)}
      </span>
      <span className="font-mono tabular-nums text-xs text-[var(--accent)] w-16 shrink-0">
        {formatDurationShort(row.duration_s)}
      </span>
      <span className="flex-1 min-w-0 truncate text-xs text-[var(--text-secondary)]">
        {comment && comment.trim().length > 0 ? (
          comment
        ) : (
          <span className="text-[var(--text-tertiary)]">{t("unassigned.noNote")}</span>
        )}
      </span>
      <IssuePicker onPick={(key) => onAssign(row, key)} />
    </li>
  );
}

/** Stable list key — `id` for persisted rows, else a started_at fallback. */
function rowKey(row: ApiWorklogRow): string {
  return row.id != null ? `id:${row.id}` : `t:${row.started_at}`;
}

export default Unassigned;

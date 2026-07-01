/**
 * Historie změn — read-only forensic view of every worklog mutation.
 *
 *   Historie změn                                  [Filtr]  [Obnovit]
 *   ┌─ Dnes ─────────────────────────────────────────────────────────┐
 *   │ [Smazáno] DEV-792 #34567                       14:32           │
 *   │ Před: 2h 15m · 15:46–18:01 · "Sync portál"                     │
 *   │ — (smazáno)                                                    │
 *   │ ✓ Úspěšně                              [Obnovit v Jira]        │
 *   └────────────────────────────────────────────────────────────────┘
 *   ┌─ Včera ────────────────────────────────────────────────────────┐
 *   │ …                                                              │
 *
 * Behaviors:
 * - Grouped by day with sticky-ish section headers (Dnes / Včera / dd. m. yyyy).
 * - 50 entries per page; "Načíst další" at the bottom drives pagination
 *   via the `beforeId` cursor.
 * - Filter pills: Vše / Smazáno / Změněno / Selhalo. Multi-select.
 * - Reconstruction buttons (Obnovit / Vrátit / Zkusit znovu) require
 *   confirmation. After a successful reconstruction, the source entry shows
 *   "Již obnoveno" instead of an active button (detected via
 *   `source_audit_id` linkage).
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useOutletContext } from "react-router-dom";

import {
  getAuditLog,
  restoreDeletedWorklog,
  retryFailedAuditAction,
  revertWorklogUpdate,
} from "../api/commands";
import type { AuditEntry } from "../api/types";
import { AuditRow } from "../components/Audit/AuditRow";
import { FilterPills, type FilterKey } from "../components/Audit/FilterPills";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { PageContainer } from "../components/Layout/PageContainer";
import { Button } from "../components/common/Button";
import { useT, type TFunc } from "../i18n";
import { formatDateCs } from "../lib/format";

const PAGE_SIZE = 50;

export default function Audit() {
  const t = useT();
  const ctx = useOutletContext<ShellOutletContext>();
  const queryClient = useQueryClient();

  const [filter, setFilter] = useState<Set<FilterKey>>(new Set(["all"]));
  const [pages, setPages] = useState<AuditEntry[][]>([]);
  const [busyId, setBusyId] = useState<number | null>(null);

  const filterArgs = useMemo(() => filterToArgs(filter), [filter]);

  const firstPageQ = useQuery({
    queryKey: ["audit-log", filterArgs],
    queryFn: () =>
      getAuditLog({
        limit: PAGE_SIZE,
        ops: filterArgs.ops,
        onlyFailed: filterArgs.onlyFailed,
      }),
  });

  // Reset paginated tail whenever the filter changes.
  const allEntries = useMemo(() => {
    const head = firstPageQ.data ?? [];
    const flat = [...head, ...pages.flat()];
    // De-dup by id (first occurrence wins).
    const seen = new Set<number>();
    const out: AuditEntry[] = [];
    for (const e of flat) {
      if (!seen.has(e.id)) {
        seen.add(e.id);
        out.push(e);
      }
    }
    return out;
  }, [firstPageQ.data, pages]);

  const reconstructedSourceIds = useMemo(() => {
    const s = new Set<number>();
    for (const e of allEntries) {
      if (
        (e.op === "restore" || e.op === "revert") &&
        e.success &&
        e.source_audit_id != null
      ) {
        s.add(e.source_audit_id);
      }
    }
    return s;
  }, [allEntries]);

  const groups = useMemo(
    () => groupByDay(allEntries, new Date(), t),
    [allEntries, t],
  );

  // Pagination: fetch the next page using the last entry's id as cursor.
  const handleLoadMore = useCallback(async () => {
    if (allEntries.length === 0) return;
    const lastId = allEntries[allEntries.length - 1].id;
    const next = await getAuditLog({
      limit: PAGE_SIZE,
      beforeId: lastId,
      ops: filterArgs.ops,
      onlyFailed: filterArgs.onlyFailed,
    });
    if (next.length > 0) {
      setPages((prev) => [...prev, next]);
    }
  }, [allEntries, filterArgs]);

  const handleFilterChange = useCallback((next: Set<FilterKey>) => {
    setFilter(next);
    setPages([]); // forget paginated tail
  }, []);

  const handleRefresh = useCallback(() => {
    setPages([]);
    queryClient.invalidateQueries({ queryKey: ["audit-log"] });
  }, [queryClient]);

  const reload = useCallback(() => {
    setPages([]);
    queryClient.invalidateQueries({ queryKey: ["audit-log"] });
  }, [queryClient]);

  const handleRestore = useCallback(
    async (entry: AuditEntry) => {
      setBusyId(entry.id);
      try {
        await restoreDeletedWorklog(entry.id);
        ctx.pushToast(
          "success",
          t("routes.audit.restoreSuccess", { issue: entry.issue_key ?? "" }),
        );
        reload();
      } catch (e) {
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : t("routes.audit.restoreFailed"),
        );
      } finally {
        setBusyId(null);
      }
    },
    [ctx, reload, t],
  );

  const handleRevert = useCallback(
    async (entry: AuditEntry) => {
      setBusyId(entry.id);
      try {
        await revertWorklogUpdate(entry.id);
        ctx.pushToast(
          "success",
          t("routes.audit.revertSuccess", { issue: entry.issue_key ?? "" }),
        );
        reload();
      } catch (e) {
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : t("routes.audit.revertFailed"),
        );
      } finally {
        setBusyId(null);
      }
    },
    [ctx, reload, t],
  );

  const handleRetry = useCallback(
    async (entry: AuditEntry) => {
      setBusyId(entry.id);
      try {
        await retryFailedAuditAction(entry.id);
        ctx.pushToast("success", t("routes.audit.retrySuccess"));
        reload();
      } catch (e) {
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : t("routes.audit.retryFailed"),
        );
      } finally {
        setBusyId(null);
      }
    },
    [ctx, reload, t],
  );

  return (
    <PageContainer maxWidth="max-w-[920px]" gap="gap-4">
      <header className="flex items-baseline justify-between gap-3 flex-wrap pt-2">
        <h1 className="text-xl font-semibold text-[var(--text-primary)]">
          {t("routes.audit.title")}
        </h1>
        <div className="flex items-center gap-2">
          <FilterPills active={filter} onChange={handleFilterChange} />
          <Button
            variant="secondary"
            size="sm"
            onClick={handleRefresh}
            aria-label={t("routes.audit.refreshHistory")}
          >
            <RefreshCw className="w-3.5 h-3.5" aria-hidden />
            {t("routes.audit.refresh")}
          </Button>
        </div>
      </header>

      {firstPageQ.isLoading && (
        <div className="text-xs text-[var(--text-tertiary)] py-2">
          {t("routes.audit.loading")}
        </div>
      )}

      {!firstPageQ.isLoading && allEntries.length === 0 && (
        <div
          className="text-xs text-[var(--text-tertiary)] py-6 text-center
                     rounded-[var(--radius-md)] border border-dashed border-[var(--border-subtle)]"
        >
          {t("routes.audit.empty")}
        </div>
      )}

      {groups.map((g) => (
        <section key={g.key} className="flex flex-col gap-2">
          <h2
            className="text-[11px] uppercase tracking-[0.08em]"
            style={{ color: "var(--text-tertiary)" }}
          >
            {g.label}
          </h2>
          <div className="flex flex-col gap-2">
            {g.entries.map((e) => (
              <AuditRow
                key={e.id}
                entry={e}
                alreadyReconstructed={reconstructedSourceIds.has(e.id)}
                busy={busyId === e.id}
                onRestore={handleRestore}
                onRevert={handleRevert}
                onRetry={handleRetry}
              />
            ))}
          </div>
        </section>
      ))}

      {allEntries.length > 0 && allEntries.length % PAGE_SIZE === 0 && (
        <div className="flex justify-center">
          <Button variant="secondary" size="md" onClick={handleLoadMore}>
            {t("routes.audit.loadMore")}
          </Button>
        </div>
      )}
    </PageContainer>
  );
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

interface DayGroup {
  key: string;
  label: string;
  entries: AuditEntry[];
}

/**
 * Group audit entries by their calendar day (in the user's local TZ). Result
 * preserves entry order within each group (newest first, since the entries
 * come back DESC).
 */
export function groupByDay(
  entries: AuditEntry[],
  now: Date = new Date(),
  t?: TFunc,
): DayGroup[] {
  const groups: DayGroup[] = [];
  const dayKey = (d: Date) =>
    `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  const todayLabel = t ? t("routes.audit.today") : "Dnes";
  const yesterdayLabel = t ? t("routes.audit.yesterday") : "Včera";

  const today = startOfDay(now);
  const todayKey = dayKey(today);
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  const yesterdayKey = dayKey(yesterday);

  for (const e of entries) {
    const dt = new Date(e.occurred_at * 1000);
    const k = dayKey(dt);
    let label: string;
    if (k === todayKey) label = todayLabel;
    else if (k === yesterdayKey) label = yesterdayLabel;
    else label = formatDateCs(dt);
    const last = groups[groups.length - 1];
    if (last && last.key === k) {
      last.entries.push(e);
    } else {
      groups.push({ key: k, label, entries: [e] });
    }
  }
  return groups;
}

function startOfDay(d: Date): Date {
  const out = new Date(d);
  out.setHours(0, 0, 0, 0);
  return out;
}

/** Translate the UI filter pill state into a backend filter payload. */
function filterToArgs(active: Set<FilterKey>): {
  ops: string[] | null;
  onlyFailed: boolean | null;
} {
  if (active.has("all")) {
    return { ops: null, onlyFailed: null };
  }
  const ops: string[] = [];
  if (active.has("delete")) ops.push("delete", "sync_tombstone");
  if (active.has("update")) ops.push("update");
  return {
    ops: ops.length > 0 ? ops : null,
    onlyFailed: active.has("failed") ? true : null,
  };
}

/**
 * Časový záznam (Time Log) — home screen.
 *
 *   Časový záznam  [Dnes ▾]  14. 5. 2026 → 14. 5. 2026                5h 21m
 *                                                                       Celkem
 *
 *   ┌─ Časová osa dne ────────────────────────────────────────────────────┐
 *   │  06  07  08  09  ██  ██  12  ██  ██  15  16  ██  17  ██  19  20  21 │
 *   └────────────────────────────────────────────────────────────────────────┘
 *
 *   ┌─ DEV-792  Portál – Synchronizace…   14. 5. 2026  15:46 – 16:01  15m ┐
 *   …
 *
 * Phase 15 — every row now supports:
 *   - Inline edit of start time, end time and duration (click → edit → blur).
 *   - Inline edit of the comment.
 *   - Soft-delete via the trash icon: the row is optimistically hidden and
 *     a "Vrátit" undo toast appears for 5 seconds. After 5s the backend
 *     fires the real Jira DELETE.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, MessageSquare, Plus, Trash2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useOutletContext } from "react-router-dom";

import {
  deleteWorklog,
  deleteLocalOnlyWorklog,
  getWorklogsForRange,
  undoDeleteWorklog,
  updateWorklog,
} from "../api/commands";
import type { WorklogRow as ApiWorklogRow } from "../api/types";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { IssuePill } from "../components/common/IssuePill";
import { DayTimeline } from "../components/Timer/DayTimeline";
import {
  addDays,
  dayEndUnixS,
  dayStartUnixS,
  startOfDay,
  startOfWeek,
} from "../lib/dates";
import { formatDateCs, formatDurationShort } from "../lib/format";
import { useTodayBoundary } from "../hooks/useTodayBoundary";
import { usePrefsStore } from "../stores/prefsStore";

type Period = "today" | "yesterday" | "this-week";

const PERIOD_LABEL: Record<Period, string> = {
  today: "Dnes",
  yesterday: "Včera",
  "this-week": "Tento týden",
};

export default function TimeLog() {
  const ctx = useOutletContext<ShellOutletContext>();
  const queryClient = useQueryClient();
  const [period, setPeriod] = useState<Period>("today");
  const dayTimelineVisible = usePrefsStore((s) => s.dayTimelineVisible);

  // Phase 18A — Item 9: re-evaluate the period range when the day rolls
  // over so a long-open Today view doesn't keep showing yesterday's date.
  const dayBoundary = useTodayBoundary();

  /** Rows the user just clicked "delete" on — optimistically hidden. */
  const [hiddenIds, setHiddenIds] = useState<Set<string>>(new Set());

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const [from, to] = useMemo(
    () => periodRange(period),
    [period, dayBoundary.rolloverCount],
  );

  const fromUnix = dayStartUnixS(from);
  const toUnix = dayEndUnixS(to);

  const worklogsQ = useQuery({
    queryKey: ["worklogs-range", fromUnix, toUnix],
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = (worklogsQ.data ?? []).filter(
    (r) => !r.jira_worklog_id || !hiddenIds.has(r.jira_worklog_id),
  );
  const totalSeconds = rows.reduce((a, r) => a + r.duration_s, 0);

  const handleDelete = useCallback(
    async (row: ApiWorklogRow) => {
      const jiraId = row.jira_worklog_id;
      // Phase 18A — Item 7: local-only rows (no Jira id) bypass the Jira
      // DELETE and are hard-deleted from the cache directly.
      if (!jiraId) {
        if (!row.id) return;
        try {
          await deleteLocalOnlyWorklog(row.id);
          queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
        } catch (e) {
          ctx.pushToast(
            "error",
            typeof e === "string" ? e : "Nepodařilo se smazat záznam",
          );
        }
        return;
      }
      // Optimistic hide.
      setHiddenIds((prev) => {
        const next = new Set(prev);
        next.add(jiraId);
        return next;
      });
      try {
        await deleteWorklog(jiraId, row.issue_key);
      } catch (e) {
        // Failure to even mark pending → un-hide + show error.
        setHiddenIds((prev) => {
          const next = new Set(prev);
          next.delete(jiraId);
          return next;
        });
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : "Záznam se nepodařilo smazat",
        );
        return;
      }
      // Show undo toast with 5s grace window.
      ctx.pushToast("info", "Záznam smazán", {
        ttlMs: 5000,
        undo: {
          label: "Vrátit",
          action: async () => {
            try {
              await undoDeleteWorklog(jiraId);
            } catch {
              /* swallow */
            } finally {
              setHiddenIds((prev) => {
                const next = new Set(prev);
                next.delete(jiraId);
                return next;
              });
              queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
            }
          },
        },
      });
    },
    [ctx, queryClient],
  );

  const handleUpdate = useCallback(
    async (
      row: ApiWorklogRow,
      patch: {
        startedAtMs?: number;
        durationSeconds?: number;
        comment?: string | null;
      },
    ) => {
      const jiraId = row.jira_worklog_id;
      if (!jiraId) {
        ctx.pushToast("error", "Tento záznam ještě nebyl synchronizován do Jiry.");
        return;
      }
      try {
        await updateWorklog({
          worklogId: jiraId,
          issueKey: row.issue_key,
          newStartedAtMs: patch.startedAtMs ?? null,
          newDurationSeconds: patch.durationSeconds ?? null,
          newComment: patch.comment ?? null,
        });
        queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
      } catch (e) {
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : "Záznam se nepodařilo aktualizovat",
        );
      }
    },
    [ctx, queryClient],
  );

  return (
    <div className="px-6 pb-6 pt-2 flex flex-col gap-5 w-full max-w-[1100px] mx-auto">
      {/* Header row ----------------------------------------------------- */}
      <div className="flex items-baseline justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-baseline gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Časový záznam
          </h1>
          <PeriodSelector value={period} onChange={setPeriod} />
          <span className="text-xs font-mono text-[var(--text-tertiary)]">
            {formatDateCs(from)} → {formatDateCs(to)}
          </span>
        </div>
        <div className="text-right">
          <div className="text-xl font-semibold text-[var(--accent)] tabular-nums">
            {totalSeconds > 0 ? formatDurationShort(totalSeconds) : "0m"}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)]">
            Celkem
          </div>
        </div>
      </div>

      {/* Day timeline (optional, user pref) ----------------------------- */}
      {dayTimelineVisible && <DayTimeline rows={rows} day={from} />}

      {/* Worklog rows ---------------------------------------------------- */}
      <div className="flex flex-col gap-1">
        {worklogsQ.isLoading && (
          <div className="text-xs text-[var(--text-tertiary)] py-2">Načítání…</div>
        )}
        {!worklogsQ.isLoading && rows.length === 0 && (
          <div className="text-xs text-[var(--text-tertiary)] py-6 text-center
                          rounded-[var(--radius-md)] border border-dashed border-[var(--border-subtle)]">
            Pro toto období nejsou žádné záznamy. Stiskněte{" "}
            <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘N</kbd>{" "}
            pro přidání.
          </div>
        )}
        {[...rows]
          .sort((a, b) => b.started_at - a.started_at)
          .map((r) => (
            <WorklogRow
              key={r.id ?? `${r.issue_key}-${r.started_at}`}
              row={r}
              onUpdate={handleUpdate}
              onDelete={handleDelete}
            />
          ))}
      </div>

      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => ctx.openAddEntry?.()}
          className="inline-flex items-center gap-1.5 px-3.5 h-9
                     rounded-[var(--radius-md)] text-[13px] font-medium
                     bg-[var(--accent)] text-[var(--accent-text)]
                     hover:bg-[var(--bg-hover)]
                     transition-colors duration-150"
        >
          <Plus className="w-3.5 h-3.5" aria-hidden />
          Nový záznam
        </button>
      </div>
    </div>
  );
}

function PeriodSelector({
  value,
  onChange,
}: {
  value: Period;
  onChange: (p: Period) => void;
}) {
  return (
    <label className="inline-flex items-center gap-1 cursor-pointer">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as Period)}
        className="appearance-none bg-transparent border-none text-sm text-[var(--text-secondary)]
                   cursor-pointer focus:outline-none pr-4"
        aria-label="Období"
      >
        {(["today", "yesterday", "this-week"] as Period[]).map((p) => (
          <option key={p} value={p}>
            {PERIOD_LABEL[p]}
          </option>
        ))}
      </select>
      <ChevronDown
        className="w-3 h-3 -ml-3 text-[var(--text-tertiary)] pointer-events-none"
        aria-hidden
      />
    </label>
  );
}

interface WorklogRowProps {
  row: ApiWorklogRow;
  onUpdate: (
    row: ApiWorklogRow,
    patch: {
      startedAtMs?: number;
      durationSeconds?: number;
      comment?: string | null;
    },
  ) => Promise<void>;
  onDelete: (row: ApiWorklogRow) => void;
}

function WorklogRow({ row, onUpdate, onDelete }: WorklogRowProps) {
  const started = new Date(row.started_at * 1000);
  const ended = new Date((row.started_at + row.duration_s) * 1000);

  const [editing, setEditing] = useState<"start" | "end" | "duration" | "comment" | null>(
    null,
  );
  const [draftStart, setDraftStart] = useState(formatHHMM(started));
  const [draftEnd, setDraftEnd] = useState(formatHHMM(ended));
  const [draftDuration, setDraftDuration] = useState(formatDurationShort(row.duration_s));
  const [draftComment, setDraftComment] = useState(row.comment ?? "");

  // When `row` changes (after a successful update) re-sync drafts.
  useMemo(() => {
    setDraftStart(formatHHMM(started));
    setDraftEnd(formatHHMM(ended));
    setDraftDuration(formatDurationShort(row.duration_s));
    setDraftComment(row.comment ?? "");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row.started_at, row.duration_s, row.comment]);

  const commitStart = async () => {
    setEditing(null);
    const newDt = combineDateAndTime(started, draftStart);
    if (!newDt || newDt.getTime() === started.getTime()) return;
    await onUpdate(row, { startedAtMs: newDt.getTime() });
  };

  const commitEnd = async () => {
    setEditing(null);
    const newEnd = combineDateAndTime(started, draftEnd);
    if (!newEnd) return;
    const newDur = Math.max(0, Math.round((newEnd.getTime() - started.getTime()) / 1000));
    if (newDur === row.duration_s) return;
    await onUpdate(row, { durationSeconds: newDur });
  };

  const commitDuration = async () => {
    setEditing(null);
    const seconds = parseDurationToSeconds(draftDuration);
    if (seconds === null || seconds === row.duration_s) return;
    await onUpdate(row, { durationSeconds: seconds });
  };

  const commitComment = async () => {
    setEditing(null);
    if (draftComment === (row.comment ?? "")) return;
    await onUpdate(row, { comment: draftComment.length > 0 ? draftComment : null });
  };

  return (
    <div
      className="flex items-center gap-3 h-12 px-3 rounded-[var(--radius-md)]
                 bg-[var(--bg-surface)] border border-[var(--border-subtle)]
                 hover:bg-[var(--bg-hover)] transition-colors duration-150"
    >
      <IssuePill issueKey={row.issue_key} />
      {editing === "comment" ? (
        <input
          type="text"
          autoFocus
          value={draftComment}
          onChange={(e) => setDraftComment(e.target.value)}
          onBlur={commitComment}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setDraftComment(row.comment ?? "");
              setEditing(null);
            }
          }}
          placeholder="Komentář"
          className="flex-1 min-w-0 text-xs bg-transparent border border-[var(--border-subtle)]
                     rounded-[var(--radius-sm)] px-2 h-7 focus:outline-none
                     focus:border-[var(--border-default)]"
        />
      ) : (
        <button
          type="button"
          onClick={() => setEditing("comment")}
          className="flex-1 min-w-0 truncate text-xs text-left text-[var(--text-primary)]
                     hover:underline decoration-dotted underline-offset-4"
          title="Upravit komentář"
        >
          <span className="inline-flex items-center gap-1">
            {/* Phase 18A — Item 8: fall back to "(načítá se…)" instead of
                "(bez popisu)" when the summary is missing; the next sync will
                backfill it. */}
            {row.summary || "(načítá se…)"}
            {row.comment && (
              <MessageSquare
                className="w-3 h-3 text-[var(--text-tertiary)]"
                aria-hidden
              />
            )}
            {/* Phase 18A — Item 7: visual marker for local-only / unsynced
                worklogs (no Jira id). */}
            {!row.jira_worklog_id && !row.pending_assignment && (
              <span
                title="Tento záznam se nepodařilo synchronizovat s Jirou"
                className="font-mono text-[10px] text-orange-500 ml-1"
              >
                ⚠ lokální
              </span>
            )}
            {row.pending_assignment && (
              <span
                title="Časomíra byla zastavena bez přiřazeného úkolu — vyberte úkol pomocí kontextového menu"
                className="font-mono text-[10px] text-red-500 ml-1"
              >
                ⚠ bez úkolu
              </span>
            )}
          </span>
        </button>
      )}
      <span className="font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0
                       px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                       inline-flex items-center">
        {formatDateCs(started)}
      </span>
      {editing === "start" ? (
        <input
          type="time"
          autoFocus
          value={draftStart}
          onChange={(e) => setDraftStart(e.target.value)}
          onBlur={commitStart}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setDraftStart(formatHHMM(started));
              setEditing(null);
            }
          }}
          className={editCellCls}
          aria-label="Začátek"
        />
      ) : (
        <button
          type="button"
          onClick={() => setEditing("start")}
          className={readCellCls}
          title="Upravit začátek"
        >
          {formatHHMM(started)}
        </button>
      )}
      <span aria-hidden className="text-[var(--text-tertiary)]">–</span>
      {editing === "end" ? (
        <input
          type="time"
          autoFocus
          value={draftEnd}
          onChange={(e) => setDraftEnd(e.target.value)}
          onBlur={commitEnd}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setDraftEnd(formatHHMM(ended));
              setEditing(null);
            }
          }}
          className={editCellCls}
          aria-label="Konec"
        />
      ) : (
        <button
          type="button"
          onClick={() => setEditing("end")}
          className={readCellCls}
          title="Upravit konec"
        >
          {formatHHMM(ended)}
        </button>
      )}
      {editing === "duration" ? (
        <input
          type="text"
          autoFocus
          value={draftDuration}
          onChange={(e) => setDraftDuration(e.target.value)}
          onBlur={commitDuration}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setDraftDuration(formatDurationShort(row.duration_s));
              setEditing(null);
            }
          }}
          placeholder="1h 30m"
          className="font-mono tabular-nums text-[11px] shrink-0 w-16 text-right
                     bg-transparent border border-[var(--border-subtle)]
                     rounded-[var(--radius-sm)] px-1 h-7 focus:outline-none
                     focus:border-[var(--border-default)]"
          aria-label="Trvání"
        />
      ) : (
        <button
          type="button"
          onClick={() => setEditing("duration")}
          className="font-mono tabular-nums text-[11px] text-[var(--text-primary)] shrink-0
                     w-16 text-right hover:underline decoration-dotted underline-offset-4"
          title="Upravit trvání"
        >
          {formatDurationShort(row.duration_s)}
        </button>
      )}
      <button
        type="button"
        aria-label={`Smazat záznam ${row.issue_key}`}
        title="Smazat"
        onClick={() => onDelete(row)}
        className="text-[var(--text-tertiary)] hover:text-[var(--danger)] transition-colors duration-150"
      >
        <Trash2 className="w-3.5 h-3.5" aria-hidden />
      </button>
    </div>
  );
}

const readCellCls =
  "font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] shrink-0 " +
  "px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)] " +
  "inline-flex items-center hover:bg-[var(--bg-hover)] " +
  "transition-colors duration-150";

const editCellCls =
  "font-mono tabular-nums text-[11px] shrink-0 " +
  "px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-default)] " +
  "bg-transparent focus:outline-none";

function periodRange(p: Period): [Date, Date] {
  const today = startOfDay(new Date());
  if (p === "today") return [today, today];
  if (p === "yesterday") {
    const y = addDays(today, -1);
    return [y, y];
  }
  // This week
  const monday = startOfWeek(today);
  return [monday, today];
}

function formatHHMM(d: Date): string {
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

/**
 * Combine the date part of `baseDate` with a `HH:MM` time string into a fresh
 * `Date`. Returns `null` if the time string is invalid.
 */
function combineDateAndTime(baseDate: Date, hhmm: string): Date | null {
  const m = /^(\d{1,2}):(\d{1,2})$/.exec(hhmm);
  if (!m) return null;
  const h = parseInt(m[1], 10);
  const mm = parseInt(m[2], 10);
  if (h < 0 || h > 23 || mm < 0 || mm > 59) return null;
  return new Date(
    baseDate.getFullYear(),
    baseDate.getMonth(),
    baseDate.getDate(),
    h,
    mm,
    0,
    0,
  );
}

/**
 * Parse a free-form duration string (`"1h 30m"`, `"90m"`, `"2h"`, `"45"`) into
 * seconds. Returns `null` if no recognizable parts are present.
 */
export function parseDurationToSeconds(s: string): number | null {
  const trimmed = s.trim().toLowerCase();
  if (!trimmed) return null;
  let total = 0;
  let matched = false;
  const hrRe = /(\d+(?:[.,]\d+)?)\s*h/g;
  const minRe = /(\d+(?:[.,]\d+)?)\s*m/g;
  for (const m of trimmed.matchAll(hrRe)) {
    total += parseFloat(m[1].replace(",", ".")) * 3600;
    matched = true;
  }
  for (const m of trimmed.matchAll(minRe)) {
    total += parseFloat(m[1].replace(",", ".")) * 60;
    matched = true;
  }
  if (!matched) {
    // Bare integer = minutes (common shorthand).
    const bare = parseInt(trimmed, 10);
    if (Number.isFinite(bare)) {
      total = bare * 60;
      matched = true;
    }
  }
  if (!matched) return null;
  return Math.round(total);
}

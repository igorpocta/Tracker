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
import { ChevronDown, ChevronLeft, ChevronRight, MessageSquare, Plus, Trash2 } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
import { useLocation, useOutletContext } from "react-router-dom";

import { invalidateWorklogQueries, queryKeys } from "../api/queryKeys";
import {
  assignWorklogIssue,
  createManualWorklog,
  deleteWorklog,
  deleteLocalOnlyWorklog,
  getWorklogsForRange,
  pushLocalWorklog,
  splitWorklog,
  undoDeleteWorklog,
  updateLocalWorklog,
  updateWorklog,
} from "../api/commands";
import type { WorklogRow as ApiWorklogRow } from "../api/types";
import type { ShellOutletContext } from "../components/Layout/AppShell";
import { PageContainer } from "../components/Layout/PageContainer";
import { IssuePill } from "../components/common/IssuePill";
import { IssuePicker } from "../components/Worklog/IssuePicker";
import { DayTimeline } from "../components/Timer/DayTimeline";
import { SuggestionBanner } from "../components/Timer/SuggestionBanner";
import {
  addDays,
  combineDateAndTime,
  dayEndUnixS,
  dayStartUnixS,
  formatHHMM,
  startOfDay,
  startOfWeek,
} from "../lib/dates";
import { formatDateCs, formatDurationShort } from "../lib/format";
import { useTodayBoundary } from "../hooks/useTodayBoundary";
import { usePrefsStore } from "../stores/prefsStore";

type Mode = "day" | "week";

function worklogUiKey(row: ApiWorklogRow): string {
  if (row.id != null) return `local:${row.id}`;
  if (row.jira_worklog_id) return `remote:${row.jira_worklog_id}`;
  return `started:${row.issue_key ?? "none"}:${row.started_at}`;
}

export default function TimeLog() {
  const ctx = useOutletContext<ShellOutletContext>();
  const queryClient = useQueryClient();
  // The Calendar route navigates here with `location.state.targetDateMs` set
  // when the user picks "Detail dne" from the right-click context menu. We
  // honor that as the initial selection so the user lands on the right day.
  const location = useLocation();
  const initialDate = useMemo<Date>(() => {
    const stateTs = (location.state as { targetDateMs?: number } | null)?.targetDateMs;
    if (typeof stateTs === "number" && Number.isFinite(stateTs)) {
      return startOfDay(new Date(stateTs));
    }
    return startOfDay(new Date());
    // Only consume the state on first mount; subsequent renders use the
    // current `selectedDate` state instead.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const [mode, setMode] = useState<Mode>("day");
  const [selectedDate, setSelectedDate] = useState<Date>(initialDate);
  const dayTimelineVisible = usePrefsStore((s) => s.dayTimelineVisible);

  // Phase 18A — Item 9: re-evaluate the period range when the day rolls
  // over so a long-open Today view doesn't keep showing yesterday's date.
  const dayBoundary = useTodayBoundary();

  /** Rows the user just clicked "delete" on — optimistically hidden. */
  const [hiddenIds, setHiddenIds] = useState<Set<string>>(new Set());
  /** Phase 18B — Item 31: row id flashed by the day-timeline click. */
  const [highlightId, setHighlightId] = useState<string | null>(null);
  const [splitRequest, setSplitRequest] = useState<{
    row: ApiWorklogRow;
    splitAtMs: number;
  } | null>(null);
  const [createRequest, setCreateRequest] = useState<{
    startedAtMs: number;
    endedAtMs: number;
  } | null>(null);
  const rowRefs = useRef<Record<string, HTMLDivElement | null>>({});

  /** Today, recomputed when the day rolls over so "Dnes" / disabled-next stay accurate. */
  const todayStart = useMemo(
    () => startOfDay(new Date()),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [dayBoundary.rolloverCount],
  );

  const [from, to] = useMemo<[Date, Date]>(() => {
    if (mode === "week") {
      const monday = startOfWeek(selectedDate);
      return [monday, addDays(monday, 6)];
    }
    return [selectedDate, selectedDate];
  }, [mode, selectedDate]);

  /** True iff the selected day equals today (used to disable "next" + hide "Dnes" pill). */
  const isAtToday = selectedDate.getTime() === todayStart.getTime();

  const handlePrev = useCallback(() => {
    setSelectedDate((d) => addDays(d, mode === "week" ? -7 : -1));
  }, [mode]);

  const handleNext = useCallback(() => {
    setSelectedDate((d) => {
      const next = addDays(d, mode === "week" ? 7 : 1);
      // Never advance past today.
      return next > todayStart ? todayStart : next;
    });
  }, [mode, todayStart]);

  const handleJumpToday = useCallback(() => {
    setSelectedDate(todayStart);
  }, [todayStart]);

  /** Label shown in the header (e.g. "Dnes · čt 14.5." or "po 12.5. – ne 18.5."). */
  const headerLabel = useMemo(() => {
    if (mode === "week") {
      return `${formatDateCs(from)} – ${formatDateCs(to)}`;
    }
    const diffDays = Math.round(
      (selectedDate.getTime() - todayStart.getTime()) / (24 * 3600 * 1000),
    );
    let prefix: string | null = null;
    if (diffDays === 0) prefix = "Dnes";
    else if (diffDays === -1) prefix = "Včera";
    else if (diffDays === 1) prefix = "Zítra";
    const fmt = formatDateCs(selectedDate);
    return prefix ? `${prefix} · ${fmt}` : fmt;
  }, [mode, selectedDate, todayStart, from, to]);

  const fromUnix = dayStartUnixS(from);
  const toUnix = dayEndUnixS(to);

  const worklogsQ = useQuery({
    queryKey: queryKeys.worklogs.range(fromUnix, toUnix),
    queryFn: () => getWorklogsForRange(fromUnix, toUnix),
  });

  const rows = (worklogsQ.data ?? []).filter((r) => !hiddenIds.has(worklogUiKey(r)));
  const totalSeconds = rows.reduce((a, r) => a + r.duration_s, 0);

  const handleDelete = useCallback(
    async (row: ApiWorklogRow) => {
      const jiraId = row.jira_worklog_id;
      const hiddenKey = worklogUiKey(row);
      // Phase 18A — Item 7: local-only rows (no Jira id) bypass the Jira
      // DELETE and are hard-deleted from the cache directly.
      if (!jiraId) {
        if (!row.id) return;
        try {
          await deleteLocalOnlyWorklog(row.id);
          invalidateWorklogQueries(queryClient);
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
        next.add(hiddenKey);
        return next;
      });
      try {
        await deleteWorklog(jiraId, row.issue_key ?? "");
      } catch (e) {
        // Failure to even mark pending → un-hide + show error.
        setHiddenIds((prev) => {
          const next = new Set(prev);
          next.delete(hiddenKey);
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
                next.delete(hiddenKey);
                return next;
              });
              invalidateWorklogQueries(queryClient);
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
      // `jira_worklog_id` is a misnomer left over from the single-provider
      // era — it holds the upstream ID for ALL providers: a Jira id for
      // Jira worklogs, `freelo:<id>` for Freelo. When it's null the row is
      // local-only; we update the SQLite row directly via `updateLocalWorklog`
      // and skip the upstream push. The next sync flushes everything.
      const remoteId = row.jira_worklog_id;
      try {
        if (remoteId) {
          await updateWorklog({
            worklogId: remoteId,
            issueKey: row.issue_key ?? "",
            newStartedAtMs: patch.startedAtMs ?? null,
            newDurationSeconds: patch.durationSeconds ?? null,
            newComment: patch.comment ?? null,
          });
        } else if (row.id != null) {
          await updateLocalWorklog({
            localId: row.id,
            newStartedAtMs: patch.startedAtMs ?? null,
            newDurationSeconds: patch.durationSeconds ?? null,
            newComment: patch.comment ?? null,
          });
        } else {
          // No upstream id AND no local rowid — shouldn't happen but bail
          // safely so we don't silently swallow the edit.
          ctx.pushToast("error", "Záznam nemá ID, nelze upravit.");
          return;
        }
        invalidateWorklogQueries(queryClient);
      } catch (e) {
        ctx.pushToast(
          "error",
          typeof e === "string" ? e : "Záznam se nepodařilo aktualizovat",
        );
      }
    },
    [ctx, queryClient],
  );

  // Assign an issue to a worklog that was created without one (timer stopped
  // unassigned, or manual entry with empty issue). Calls the backend
  // `assign_worklog_issue` command which:
  //   - sets the row's issue_key
  //   - clears pending_assignment
  //   - if a Jira/Freelo client is configured, POSTs the worklog upstream
  //     too so it stops being a "local only" record.
  const handleAssign = useCallback(
    async (row: ApiWorklogRow, issueKey: string) => {
      if (row.id == null) return;
      try {
        await assignWorklogIssue(row.id, issueKey);
        invalidateWorklogQueries(queryClient);
        ctx.pushToast?.("success", `Záznam přiřazen na ${issueKey}.`);
      } catch (e) {
        ctx.pushToast?.(
          "error",
          typeof e === "string" ? e : "Přiřazení úkolu selhalo.",
        );
      }
    },
    [ctx, queryClient],
  );

  return (
    <PageContainer>
      <SuggestionBanner />
      {/* Header row ----------------------------------------------------- */}
      <div className="flex items-center justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-center gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            Časový záznam
          </h1>
          <ModeSelector value={mode} onChange={setMode} />

          {/* < / > / Dnes navigator */}
          <div className="inline-flex items-center gap-1.5">
            <button
              type="button"
              onClick={handlePrev}
              aria-label={mode === "week" ? "Předchozí týden" : "Předchozí den"}
              title={mode === "week" ? "Předchozí týden" : "Předchozí den"}
              className="w-7 h-7 inline-flex items-center justify-center rounded-[var(--radius-sm)]
                         border border-[var(--border-subtle)] text-[var(--text-secondary)]
                         hover:bg-[var(--bg-hover)] transition-colors duration-150"
            >
              <ChevronLeft className="w-3.5 h-3.5" />
            </button>

            <span className="text-xs font-mono text-[var(--text-tertiary)] min-w-[150px] text-center">
              {headerLabel}
            </span>

            <button
              type="button"
              onClick={handleNext}
              disabled={mode === "day" ? isAtToday : false}
              aria-label={mode === "week" ? "Další týden" : "Další den"}
              title={mode === "week" ? "Další týden" : "Další den"}
              className="w-7 h-7 inline-flex items-center justify-center rounded-[var(--radius-sm)]
                         border border-[var(--border-subtle)] text-[var(--text-secondary)]
                         hover:bg-[var(--bg-hover)] transition-colors duration-150
                         disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            >
              <ChevronRight className="w-3.5 h-3.5" />
            </button>

            {!isAtToday && (
              <button
                type="button"
                onClick={handleJumpToday}
                className="ml-1 h-7 px-2 inline-flex items-center rounded-[var(--radius-sm)]
                           text-[11px] font-medium
                           bg-[var(--accent-soft)] text-[var(--accent)]
                           hover:bg-[var(--bg-hover)] transition-colors duration-150"
              >
                Dnes
              </button>
            )}
          </div>
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
      {dayTimelineVisible && (
        <DayTimeline
          rows={rows}
          day={from}
          onSelect={(row) => {
            const key = String(row.id ?? row.jira_worklog_id ?? row.started_at);
            setHighlightId(key);
            rowRefs.current[key]?.scrollIntoView({
              behavior: "smooth",
              block: "center",
            });
            window.setTimeout(() => setHighlightId(null), 1500);
          }}
          onSplitRequest={(row, splitAtMs) => {
            setSplitRequest({ row, splitAtMs });
          }}
          onCreateRequest={(startedAtMs, endedAtMs) => {
            setCreateRequest({ startedAtMs, endedAtMs });
          }}
        />
      )}
      {createRequest && (
        <CreateWorklogDialog
          startedAtMs={createRequest.startedAtMs}
          endedAtMs={createRequest.endedAtMs}
          onCancel={() => setCreateRequest(null)}
          onConfirm={async (issueKey) => {
            try {
              const durationSeconds = Math.round(
                (createRequest.endedAtMs - createRequest.startedAtMs) / 1000,
              );
              await createManualWorklog({
                issueKey: issueKey,
                startedAtMs: createRequest.startedAtMs,
                durationSeconds,
                comment: null,
              });
              invalidateWorklogQueries(queryClient);
            } catch (e) {
              ctx.pushToast(
                "error",
                typeof e === "string" ? e : "Nepodařilo se vytvořit záznam",
              );
            } finally {
              setCreateRequest(null);
            }
          }}
        />
      )}
      {splitRequest && (
        <SplitWorklogDialog
          row={splitRequest.row}
          splitAtMs={splitRequest.splitAtMs}
          onCancel={() => setSplitRequest(null)}
          onConfirm={async (newIssueKey) => {
            try {
              if (splitRequest.row.id != null) {
                await splitWorklog(
                  splitRequest.row.id,
                  splitRequest.splitAtMs,
                  newIssueKey || null,
                );
                invalidateWorklogQueries(queryClient);
              }
            } catch (e) {
              ctx.pushToast(
                "error",
                typeof e === "string" ? e : "Split selhal",
              );
            } finally {
              setSplitRequest(null);
            }
          }}
        />
      )}

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
          .map((r) => {
            const key = String(r.id ?? r.jira_worklog_id ?? r.started_at);
            return (
              <WorklogRow
                key={r.id ?? `${r.issue_key}-${r.started_at}`}
                row={r}
                onUpdate={handleUpdate}
                onDelete={handleDelete}
                onAssign={handleAssign}
                highlighted={highlightId === key}
                refCallback={(el) => {
                  rowRefs.current[key] = el;
                }}
              />
            );
          })}
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
    </PageContainer>
  );
}

function ModeSelector({
  value,
  onChange,
}: {
  value: Mode;
  onChange: (m: Mode) => void;
}) {
  return (
    <label className="inline-flex items-center gap-1 cursor-pointer">
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as Mode)}
        className="appearance-none bg-transparent border-none text-sm text-[var(--text-secondary)]
                   cursor-pointer focus:outline-none pr-4"
        aria-label="Režim"
      >
        <option value="day">Den</option>
        <option value="week">Týden</option>
      </select>
      <ChevronDown
        className="w-3 h-3 -ml-3 text-[var(--text-tertiary)] pointer-events-none"
        aria-hidden
      />
    </label>
  );
}

export interface WorklogRowProps {
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
  /** Assign an issue to an unassigned worklog row. */
  onAssign: (row: ApiWorklogRow, issueKey: string) => Promise<void>;
  /** Phase 18B — Item 31: flash the row when the user picks it from the timeline. */
  highlighted?: boolean;
  refCallback?: (el: HTMLDivElement | null) => void;
}

export function WorklogRow({
  row,
  onUpdate,
  onDelete,
  onAssign,
  highlighted,
  refCallback,
}: WorklogRowProps) {
  const started = new Date(row.started_at * 1000);
  const ended = new Date((row.started_at + row.duration_s) * 1000);

  const [editing, setEditing] = useState<"start" | "end" | "duration" | "comment" | null>(
    null,
  );
  // Drafts only matter while a cell is in edit mode — and they are
  // (re-)seeded fresh from the latest `row` props the moment the user
  // clicks into a cell via the `beginEditing*` helpers below. Outside
  // of edit mode the read-mode buttons render the row values directly.
  //
  // The pre-fix version re-synced these state variables from inside a
  // `useMemo` callback whose deps were `[row.*]` — that's a setState
  // during render, which Strict Mode runs twice (so each setState
  // would fire twice on every row change) and which React Compiler
  // would treat as a memoisation invariant violation. Lazy-seeding on
  // edit-entry sidesteps both problems and is the pattern senior
  // review recommended.
  const [draftStart, setDraftStart] = useState(formatHHMM(started));
  const [draftEnd, setDraftEnd] = useState(formatHHMM(ended));
  const [draftDuration, setDraftDuration] = useState(formatDurationShort(row.duration_s));
  const [draftComment, setDraftComment] = useState(row.comment ?? "");

  const beginEditingStart = () => {
    setDraftStart(formatHHMM(started));
    setEditing("start");
  };
  const beginEditingEnd = () => {
    setDraftEnd(formatHHMM(ended));
    setEditing("end");
  };
  const beginEditingDuration = () => {
    setDraftDuration(formatDurationShort(row.duration_s));
    setEditing("duration");
  };
  const beginEditingComment = () => {
    setDraftComment(row.comment ?? "");
    setEditing("comment");
  };

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
      ref={refCallback}
      className="flex items-center gap-3 h-12 px-3 rounded-[var(--radius-md)]
                 bg-[var(--bg-surface)] border transition-colors duration-300"
      style={{
        borderColor: highlighted
          ? "var(--accent)"
          : "var(--border-subtle)",
        background: highlighted
          ? "var(--accent-soft)"
          : "var(--bg-surface)",
      }}
    >
      {row.issue_key ? (
        <IssuePill issueKey={row.issue_key} />
      ) : (
        <IssuePicker onPick={(key) => onAssign(row, key)} />
      )}
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
          onClick={beginEditingComment}
          className="flex-1 min-w-0 flex items-center gap-2 text-xs text-left text-[var(--text-primary)]
                     hover:underline decoration-dotted underline-offset-4"
          title="Upravit komentář"
        >
          {/* Phase 18A — Item 8: fall back to "(načítá se…)" when an
              issue IS set but its summary hasn't been backfilled yet (the
              next sync will). When no issue is assigned at all, show
              "Nepřiřazen" so it's clear the row is waiting for a pick.
              `min-w-0 flex-1 truncate` lets the summary shrink so the
              icons + warning chips on the right stay visible. */}
          <span className="flex-1 min-w-0 truncate">
            {row.summary ||
              (row.issue_key ? "(načítá se…)" : "Nepřiřazen")}
          </span>
          {row.comment && (
            <MessageSquare
              className="w-3 h-3 text-[var(--text-tertiary)] shrink-0"
              aria-hidden
            />
          )}
        </button>
      )}

      {/* Status chips live OUTSIDE the comment-edit button so they can be
          interactive themselves (HTML doesn't allow nested buttons). */}
      {!row.jira_worklog_id && !row.pending_assignment && row.id != null && (
        <button
          type="button"
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            void (async () => {
              if (row.id == null) return;
              try {
                await pushLocalWorklog(row.id);
              } catch (err) {
                console.error("[push_local_worklog] failed:", err);
              }
            })();
          }}
          title="Klikni pro vynucenou synchronizaci s providerem"
          className="font-mono text-[10px] text-orange-500 shrink-0
                     hover:text-orange-400 hover:underline underline-offset-2
                     transition-colors duration-150"
        >
          ⚠ lokální · ↻
        </button>
      )}
      {row.pending_assignment && (
        <span
          title="Časomíra byla zastavena bez přiřazeného úkolu — vyberte úkol vlevo"
          className="font-mono text-[10px] text-red-500 shrink-0"
        >
          ⚠ bez úkolu
        </span>
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
          onClick={beginEditingStart}
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
          onClick={beginEditingEnd}
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
          onClick={beginEditingDuration}
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


function SplitWorklogDialog({
  row,
  splitAtMs,
  onCancel,
  onConfirm,
}: {
  row: ApiWorklogRow;
  splitAtMs: number;
  onCancel: () => void;
  onConfirm: (newIssueKey: string) => void;
}) {
  const [key, setKey] = useState("");
  const splitDate = new Date(splitAtMs);
  const hh = String(splitDate.getHours()).padStart(2, "0");
  const mm = String(splitDate.getMinutes()).padStart(2, "0");
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Rozdělit záznam"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        className="w-[420px] max-w-[92vw] p-5 rounded-[var(--radius-lg)] flex flex-col gap-3"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
        }}
      >
        <h3 className="text-base font-semibold text-[var(--text-primary)]">
          Rozdělit záznam v {hh}:{mm}
        </h3>
        <p className="text-xs text-[var(--text-secondary)]">
          První kus zůstane na úkolu <span className="font-mono">{row.issue_key ?? "(bez úkolu)"}</span>.
          Druhý kus přiřaď k jinému úkolu (nech prázdné pro 'bez úkolu').
        </p>
        <input
          type="text"
          value={key}
          autoFocus
          placeholder="DEV-792"
          onChange={(e) => setKey(e.target.value.toUpperCase().trim())}
          onKeyDown={(e) => {
            if (e.key === "Enter") onConfirm(key);
            if (e.key === "Escape") onCancel();
          }}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent
                     border border-[var(--border-default)] text-sm font-mono
                     text-[var(--text-primary)] focus:outline-none
                     focus:border-[var(--accent)]"
        />
        <div className="flex justify-end gap-2 mt-1">
          <button
            type="button"
            onClick={onCancel}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          >
            Zrušit
          </button>
          <button
            type="button"
            onClick={() => onConfirm(key)}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm font-semibold"
            style={{ background: "var(--accent)", color: "var(--accent-text, #fff)" }}
          >
            Rozdělit
          </button>
        </div>
      </div>
    </div>
  );
}



function CreateWorklogDialog({
  startedAtMs,
  endedAtMs,
  onCancel,
  onConfirm,
}: {
  startedAtMs: number;
  endedAtMs: number;
  onCancel: () => void;
  onConfirm: (issueKey: string) => void;
}) {
  const [key, setKey] = useState("");
  const start = new Date(startedAtMs);
  const end = new Date(endedAtMs);
  const startLabel = `${String(start.getHours()).padStart(2, "0")}:${String(start.getMinutes()).padStart(2, "0")}`;
  const endLabel = `${String(end.getHours()).padStart(2, "0")}:${String(end.getMinutes()).padStart(2, "0")}`;
  const durMin = Math.max(1, Math.round((endedAtMs - startedAtMs) / 60000));
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Vytvořit záznam z časové osy"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        className="w-[420px] max-w-[92vw] p-5 rounded-[var(--radius-lg)] flex flex-col gap-3"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
        }}
      >
        <h3 className="text-base font-semibold text-[var(--text-primary)]">
          Vytvořit záznam {startLabel}–{endLabel}
          <span className="text-[var(--text-tertiary)] font-normal text-sm">
            {" "}· {durMin} min
          </span>
        </h3>
        <p className="text-xs text-[var(--text-secondary)]">
          Zadej úkol — záznam bude rovnou odeslán do providera (Jira / Freelo
          podle prefixu klíče). Pro lokální placeholder nech prázdné.
        </p>
        <input
          type="text"
          value={key}
          autoFocus
          placeholder="DEV-792"
          onChange={(e) => setKey(e.target.value.toUpperCase().trim())}
          onKeyDown={(e) => {
            if (e.key === "Enter" && key) onConfirm(key);
            if (e.key === "Escape") onCancel();
          }}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent
                     border border-[var(--border-default)] text-sm font-mono
                     text-[var(--text-primary)] focus:outline-none
                     focus:border-[var(--accent)]"
        />
        <div className="flex justify-end gap-2 mt-1">
          <button
            type="button"
            onClick={onCancel}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          >
            Zrušit
          </button>
          <button
            type="button"
            disabled={!key}
            onClick={() => onConfirm(key)}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm font-semibold
                       disabled:opacity-50"
            style={{
              background: "var(--accent)",
              color: "var(--accent-text, #fff)",
            }}
          >
            Vytvořit
          </button>
        </div>
      </div>
    </div>
  );
}

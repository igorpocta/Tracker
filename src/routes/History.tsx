/**
 * History route — browse worklogs by day, with a week sparkline at the top.
 *
 * Layout:
 *   ┌──── Week sparkline (Mon..Sun) ─────────────────────────────────────┐
 *   ├──────────┬───────────────────────────────────────────────────────┤
 *   │  Day     │  Selected day worklog list                            │
 *   │  picker  │                                                       │
 *   └──────────┴───────────────────────────────────────────────────────┘
 */
import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";

import { getWorklogsForRange } from "../api/commands";
import { Button } from "../components/common/Button";
import { Card } from "../components/common/Card";
import { DayPicker } from "../components/History/DayPicker";
import { WeekSparkline } from "../components/History/WeekSparkline";
import { WorklogList } from "../components/Worklog/WorklogList";
import {
  addDays,
  dayEndUnixS,
  dayStartUnixS,
  formatLongDayLabel,
  isSameDay,
  startOfDay,
  startOfWeek,
} from "../lib/dates";
import { formatDurationShort } from "../lib/format";
import { useTimerStore } from "../stores/timerStore";

/** How many days of history to load at once (covers the picker + week chart). */
const HISTORY_WINDOW_DAYS = 30;

export default function History() {
  const today = useMemo(() => startOfDay(new Date()), []);
  const [selected, setSelected] = useState<Date>(today);

  const active = useTimerStore((s) => s.active);

  // Pull a big window once. The same data feeds the picker, the sparkline,
  // and (filtered) the right-pane list.
  const windowFrom = useMemo(
    () => Math.min(dayStartUnixS(addDays(today, -(HISTORY_WINDOW_DAYS - 1))), dayStartUnixS(startOfWeek(selected))),
    [today, selected],
  );
  const windowTo = useMemo(() => dayEndUnixS(today), [today]);

  const allQ = useQuery({
    queryKey: ["worklogs-range", windowFrom, windowTo],
    queryFn: () => getWorklogsForRange(windowFrom, windowTo),
  });

  const dayRows = useMemo(() => {
    const rows = allQ.data ?? [];
    return rows.filter((r) => isSameDay(new Date(r.started_at * 1000), selected));
  }, [allQ.data, selected]);

  const dayTotal = dayRows.reduce((a, r) => a + r.duration_s, 0);

  const goPrev = () => setSelected((d) => addDays(d, -1));
  const goNext = () => {
    const next = addDays(selected, 1);
    if (next.getTime() <= today.getTime()) setSelected(next);
  };
  const canGoNext = addDays(selected, 1).getTime() <= today.getTime();

  return (
    <div className="p-8 flex flex-col gap-5 max-w-6xl mx-auto w-full">
      <Card padding="md">
        <WeekSparkline
          rows={allQ.data ?? []}
          selected={selected}
          onSelect={setSelected}
        />
      </Card>

      <div className="grid grid-cols-12 gap-4">
        <Card padding="sm" className="col-span-12 md:col-span-4 lg:col-span-3">
          <DayPicker
            rows={allQ.data ?? []}
            selected={selected}
            count={HISTORY_WINDOW_DAYS}
            onSelect={setSelected}
          />
        </Card>

        <Card padding="none" className="col-span-12 md:col-span-8 lg:col-span-9">
          <div className="px-4 py-3 border-b border-[var(--border-subtle)] flex items-center justify-between gap-3 flex-wrap">
            <div>
              <h2 className="text-sm font-semibold text-[var(--text-primary)]">
                {formatLongDayLabel(selected)}
              </h2>
              <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
                {dayRows.length} entries ·{" "}
                <span className="text-[var(--text-secondary)] font-mono tabular-nums">
                  {dayTotal > 0 ? formatDurationShort(dayTotal) : "0m"} total
                </span>
              </p>
            </div>
            <div className="flex items-center gap-1.5">
              <Button variant="ghost" size="sm" onClick={goPrev} aria-label="Previous day">
                <ChevronLeft className="w-3.5 h-3.5" aria-hidden />
                Prev
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={goNext}
                disabled={!canGoNext}
                aria-label="Next day"
              >
                Next
                <ChevronRight className="w-3.5 h-3.5" aria-hidden />
              </Button>
              {!isSameDay(selected, today) && (
                <Button variant="secondary" size="sm" onClick={() => setSelected(today)}>
                  Today
                </Button>
              )}
            </div>
          </div>
          <WorklogList
            rows={dayRows}
            loading={allQ.isLoading}
            activeIssueKey={active?.issue_key ?? null}
            emptyTitle={
              isSameDay(selected, today)
                ? "No worklogs yet today"
                : "No worklogs on this day"
            }
            emptyDescription={
              isSameDay(selected, today)
                ? "Head to Today to start your first timer."
                : "Pick another day from the rail to explore."
            }
          />
        </Card>
      </div>
    </div>
  );
}

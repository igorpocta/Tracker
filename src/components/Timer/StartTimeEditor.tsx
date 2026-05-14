/**
 * Inline editor for adjusting the active timer's start time. Two ways to
 * adjust:
 *   1. Plus/minus buttons in 5- or 1-minute increments.
 *   2. A `<input type="time">` for picking an absolute HH:MM clock value.
 *
 * Calls `onSave(startedAtMs)` with the new wall-clock millisecond timestamp.
 * The parent is responsible for actually invoking `update_timer_start` — we
 * keep it pure so the Stop dialog can edit-without-save until the user
 * confirms.
 */
import { Minus, Plus } from "lucide-react";
import { useMemo } from "react";

import { formatClockTime } from "../../lib/format";
import { Button } from "../common/Button";
import { IconButton } from "../common/IconButton";

export interface StartTimeEditorProps {
  /** Current start time in ms since epoch. */
  startedAtMs: number;
  /** Reference "now" (ms) — clamps so the user can't push start into the future. */
  nowMs: number;
  onChange: (nextMs: number) => void;
}

export function StartTimeEditor({
  startedAtMs,
  nowMs,
  onChange,
}: StartTimeEditorProps) {
  const clampedDelta = (deltaMs: number) => {
    const next = startedAtMs + deltaMs;
    return Math.min(next, nowMs);
  };

  const timeValue = useMemo(() => {
    const d = new Date(startedAtMs);
    const hh = d.getHours().toString().padStart(2, "0");
    const mm = d.getMinutes().toString().padStart(2, "0");
    return `${hh}:${mm}`;
  }, [startedAtMs]);

  const onTimeInput = (raw: string) => {
    const m = /^(\d{1,2}):(\d{2})$/.exec(raw);
    if (!m) return;
    const h = Math.min(23, Math.max(0, Number(m[1])));
    const min = Math.min(59, Math.max(0, Number(m[2])));
    const d = new Date(startedAtMs);
    d.setHours(h, min, 0, 0);
    onChange(Math.min(d.getTime(), nowMs));
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="text-xs font-medium text-[var(--text-secondary)]">Začátek</div>
      <div className="flex items-center gap-2 flex-wrap">
        <IconButton
          aria-label="Odečíst 5 minut"
          onClick={() => onChange(clampedDelta(-5 * 60_000))}
        >
          <Minus className="w-3.5 h-3.5" aria-hidden />
          <span aria-hidden className="text-[10px] ml-0.5">
            5
          </span>
        </IconButton>
        <IconButton
          aria-label="Odečíst 1 minutu"
          onClick={() => onChange(clampedDelta(-60_000))}
        >
          <Minus className="w-3.5 h-3.5" aria-hidden />
          <span aria-hidden className="text-[10px] ml-0.5">
            1
          </span>
        </IconButton>

        <input
          type="time"
          aria-label="Čas začátku"
          value={timeValue}
          onChange={(e) => onTimeInput(e.target.value)}
          className="px-2 h-8 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)]
                     focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                     text-sm font-mono text-[var(--text-primary)] transition-colors duration-150"
        />
        <span className="text-xs text-[var(--text-tertiary)]">
          ({formatClockTime(startedAtMs)})
        </span>

        <IconButton
          aria-label="Přidat 1 minutu"
          onClick={() => onChange(clampedDelta(60_000))}
        >
          <Plus className="w-3.5 h-3.5" aria-hidden />
          <span aria-hidden className="text-[10px] ml-0.5">
            1
          </span>
        </IconButton>
        <IconButton
          aria-label="Přidat 5 minut"
          onClick={() => onChange(clampedDelta(5 * 60_000))}
        >
          <Plus className="w-3.5 h-3.5" aria-hidden />
          <span aria-hidden className="text-[10px] ml-0.5">
            5
          </span>
        </IconButton>

        <Button
          variant="ghost"
          size="sm"
          onClick={() => onChange(nowMs)}
          aria-label="Resetovat začátek na teď"
        >
          Teď
        </Button>
      </div>
    </div>
  );
}

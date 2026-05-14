/**
 * Top "Start tracking…" bar — visible across every main route.
 *
 * Reference: `screens/SCR-20260514-rjbm-2.png` and friends.
 *
 *   ┌──────────────────────────────────────────┬───────┬──────────┐
 *   │  Start tracking…                         │ 19:57 │ ▶ Start  │
 *   └──────────────────────────────────────────┴───────┴──────────┘
 *
 * - Input is the issue search ("type to filter"). Typing reveals a dropdown
 *   of cached issues; picking one runs `onPickIssue(issueKey)`.
 * - Live clock to the right of the input, updates every second.
 * - "Start" button is enabled only when a query matches a single issue
 *   exactly OR the user has selected a row in the dropdown.
 * - When the timer is running, the bar shows the running issue + elapsed
 *   time + a "Stop" button instead.
 */
import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { Play, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { searchIssuesCache } from "../../api/commands";
import type { ActiveTimerState } from "../../api/types";
import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";

export interface StartTrackingBarProps {
  onPickIssue: (issueKey: string) => void;
  onStop?: () => void;
}

const LIMIT = 20;
/** Debounce time for the issue search query, in ms. */
const DEBOUNCE_MS = 120;

export function StartTrackingBar({ onPickIssue, onStop }: StartTrackingBarProps) {
  const active = useTimerStore((s) => s.active);
  const busy = useTimerStore((s) => s.busy);

  if (active) {
    return <RunningBar active={active} busy={busy} onStop={onStop} />;
  }

  return <IdleBar onPickIssue={onPickIssue} />;
}

// -----------------------------------------------------------------------------
// Idle state — search + start.
// -----------------------------------------------------------------------------

function IdleBar({ onPickIssue }: { onPickIssue: (issueKey: string) => void }) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const now = useNow(60_000);

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(t);
  }, [query]);

  const q = useQuery({
    queryKey: ["search-issues", debounced, LIMIT],
    queryFn: () => searchIssuesCache(debounced, LIMIT),
    enabled: debounced.length > 0,
  });

  const results = q.data ?? [];

  // Close the dropdown on outside click.
  useEffect(() => {
    if (!open) return;
    function onClick(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [open]);

  const handlePick = (issueKey: string) => {
    setQuery("");
    setDebounced("");
    setOpen(false);
    onPickIssue(issueKey);
  };

  const onSubmit = () => {
    if (results[highlight]) {
      handlePick(results[highlight].issue_key);
    }
  };

  const clock = formatClock(now);

  return (
    <div className="flex items-stretch gap-2" ref={containerRef}>
      <div className="relative flex-1 min-w-0">
        <input
          type="text"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
            setHighlight(0);
          }}
          onFocus={() => setOpen(query.length > 0)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setOpen(false);
              (e.currentTarget as HTMLInputElement).blur();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setHighlight((h) => Math.min(results.length - 1, h + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setHighlight((h) => Math.max(0, h - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              onSubmit();
            }
          }}
          placeholder="Start tracking…"
          aria-label="Search and start a timer"
          aria-expanded={open}
          className="w-full h-11 pl-4 pr-24 rounded-[var(--radius-md)]
                     bg-[var(--bg-surface)] border border-[var(--border-subtle)]
                     text-sm text-[var(--text-primary)]
                     placeholder:text-[var(--text-tertiary)]
                     focus:outline-none focus:border-[var(--border-default)]
                     transition-colors duration-150"
        />
        <span className="absolute right-3 top-1/2 -translate-y-1/2 font-mono tabular-nums text-xs text-[var(--text-tertiary)]">
          {clock}
        </span>

        {open && debounced.length > 0 && (
          <SearchDropdown
            results={results}
            highlight={highlight}
            onPick={handlePick}
            onHover={setHighlight}
            loading={q.isFetching && results.length === 0}
          />
        )}
      </div>

      <button
        type="button"
        onClick={onSubmit}
        disabled={results.length === 0}
        className={clsx(
          "shrink-0 inline-flex items-center justify-center gap-1.5 px-4 h-11 rounded-[var(--radius-md)]",
          "border border-[var(--border-subtle)] text-sm",
          "transition-colors duration-150",
          results.length === 0
            ? "text-[var(--text-tertiary)] cursor-not-allowed"
            : "text-[var(--text-primary)] hover:bg-[var(--bg-hover)]",
        )}
        aria-label="Start timer for selected issue"
      >
        <Play className="w-3.5 h-3.5" aria-hidden />
        Start
      </button>
    </div>
  );
}

function SearchDropdown({
  results,
  highlight,
  onPick,
  onHover,
  loading,
}: {
  results: import("../../api/types").IssueRow[];
  highlight: number;
  onPick: (key: string) => void;
  onHover: (idx: number) => void;
  loading: boolean;
}) {
  return (
    <div
      role="listbox"
      className="absolute left-0 right-0 top-full mt-1 z-30
                 rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] shadow-[var(--shadow-md)]
                 max-h-[420px] overflow-y-auto"
    >
      {loading && (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">
          Searching…
        </div>
      )}
      {!loading && results.length === 0 && (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">
          No matching issues.
        </div>
      )}
      {results.map((iss, idx) => (
        <button
          key={iss.issue_key}
          type="button"
          role="option"
          aria-selected={idx === highlight}
          onMouseEnter={() => onHover(idx)}
          onMouseDown={(e) => {
            // Use mousedown so the click registers before the outside-click
            // listener fires on the input losing focus.
            e.preventDefault();
            onPick(iss.issue_key);
          }}
          className={clsx(
            "w-full text-left flex items-center gap-2 px-3 py-2 text-xs",
            idx === highlight
              ? "bg-[var(--bg-hover)] text-[var(--text-primary)]"
              : "text-[var(--text-secondary)]",
          )}
        >
          <span className="font-mono uppercase text-[11px] text-[var(--text-tertiary)] w-20 shrink-0">
            {iss.issue_key}
          </span>
          <span className="truncate flex-1 text-[var(--text-primary)]">
            {iss.summary || "(no summary)"}
          </span>
        </button>
      ))}
    </div>
  );
}

// -----------------------------------------------------------------------------
// Running state — show elapsed + stop button.
// -----------------------------------------------------------------------------

function RunningBar({
  active,
  busy,
  onStop,
}: {
  active: ActiveTimerState;
  busy: boolean;
  onStop?: () => void;
}) {
  const now = useNow(1000);
  const elapsed = elapsedSeconds(active, now);

  return (
    <div className="flex items-stretch gap-2">
      <div className="flex-1 min-w-0 relative h-11 rounded-[var(--radius-md)]
                      bg-[var(--bg-surface)] border border-[var(--border-subtle)]
                      flex items-center px-4 gap-3">
        <span
          aria-hidden
          className="w-2 h-2 rounded-full bg-[var(--accent)] animate-pulse shrink-0"
        />
        <span className="font-mono text-[11px] uppercase tracking-[0.08em] text-[var(--accent)] shrink-0">
          {active.issue_key}
        </span>
        <span className="text-xs text-[var(--text-tertiary)] truncate">
          Tracking…
        </span>
        <span className="ml-auto font-mono tabular-nums text-sm text-[var(--accent)]">
          {formatDuration(elapsed)}
        </span>
      </div>
      <button
        type="button"
        onClick={onStop}
        disabled={busy || !onStop}
        className="shrink-0 inline-flex items-center justify-center gap-1.5 px-4 h-11
                   rounded-[var(--radius-md)] border border-[var(--accent-soft)]
                   bg-[var(--accent-soft)] text-[var(--accent)] text-sm
                   hover:bg-[var(--accent-strong)] transition-colors duration-150
                   disabled:opacity-60 disabled:cursor-not-allowed"
      >
        <Square className="w-3.5 h-3.5" aria-hidden />
        Stop
      </button>
    </div>
  );
}

function formatClock(now: number): string {
  const d = new Date(now);
  const hh = `${d.getHours()}`.padStart(2, "0");
  const mm = `${d.getMinutes()}`.padStart(2, "0");
  return `${hh}:${mm}`;
}

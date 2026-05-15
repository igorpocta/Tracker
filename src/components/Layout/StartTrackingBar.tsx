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
import { MessageSquare, Play, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { listFavorites, searchIssuesCache } from "../../api/commands";
import type { ActiveTimerState } from "../../api/types";
import { FavoriteStar } from "../Favorites/FavoriteStar";
import { useNow } from "../../hooks/useNow";
import { formatDuration } from "../../lib/format";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";

export interface StartTrackingBarProps {
  /**
   * Called when the user clicks Start. `comment` is the optional in-flight
   * note from the comment input (empty string when blank).
   */
  onPickIssue: (issueKey: string, comment: string) => void;
  onStop?: () => void;
  /**
   * Phase 18A — Item 4: callback to start an unassigned timer (empty issue
   * key). When omitted, the "Bez úkolu" button is hidden.
   */
  onStartUnassigned?: (comment: string) => void;
}

const LIMIT = 20;
/** Debounce time for the issue search query, in ms. */
const DEBOUNCE_MS = 120;

export function StartTrackingBar({
  onPickIssue,
  onStop,
  onStartUnassigned,
}: StartTrackingBarProps) {
  const active = useTimerStore((s) => s.active);
  const busy = useTimerStore((s) => s.busy);

  if (active) {
    return <RunningBar active={active} busy={busy} onStop={onStop} />;
  }

  return <IdleBar onPickIssue={onPickIssue} onStartUnassigned={onStartUnassigned} />;
}

// -----------------------------------------------------------------------------
// Idle state — search + start.
// -----------------------------------------------------------------------------

function IdleBar({
  onPickIssue,
  onStartUnassigned,
}: {
  onPickIssue: (issueKey: string, comment: string) => void;
  onStartUnassigned?: (comment: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [comment, setComment] = useState("");
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Phase 18A — Item 10: tick every second so the displayed wall clock
  // matches what gets recorded when the user clicks Start. Previously
  // this ticked at 60s, which left a perceived ~58s offset between the
  // visible time and the timer's actual start.
  const now = useNow(1000);

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(t);
  }, [query]);

  const q = useQuery({
    queryKey: ["search-issues", debounced, LIMIT],
    queryFn: () => searchIssuesCache(debounced, LIMIT),
    enabled: debounced.length > 0,
  });

  // Phase 18B — Item 26: favorites are surfaced at the top of the dropdown.
  const favoritesQ = useQuery({
    queryKey: ["favorites"],
    queryFn: listFavorites,
    staleTime: 30_000,
  });
  const favorites = favoritesQ.data ?? [];
  const favoriteKeys = new Set(favorites.map((f) => f.issue_key));

  // Merge: favorites first (matched against the query if any), then the rest
  // of the search results, de-duplicated.
  const baseResults = q.data ?? [];
  const filteredFavorites = debounced
    ? favorites.filter(
        (f) =>
          f.issue_key.toLowerCase().includes(debounced.toLowerCase()) ||
          (f.summary ?? "").toLowerCase().includes(debounced.toLowerCase()),
      )
    : favorites;
  const merged = [
    ...filteredFavorites,
    ...baseResults.filter((r) => !favoriteKeys.has(r.issue_key)),
  ];
  const results = merged;

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
    const c = comment.trim();
    setComment("");
    onPickIssue(issueKey, c);
  };

  const onSubmit = () => {
    if (results[highlight]) {
      handlePick(results[highlight].issue_key);
    }
  };

  const onStartBlank = () => {
    if (!onStartUnassigned) return;
    const c = comment.trim();
    setComment("");
    onStartUnassigned(c);
  };

  const clock = formatClock(now);

  return (
    <div className="flex items-stretch gap-2 flex-wrap" ref={containerRef}>
      <div className="relative flex-1 min-w-[260px]">
        <input
          type="text"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
            setHighlight(0);
          }}
          onFocus={() => setOpen(true)}
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
          placeholder="Začít stopovat…"
          aria-label="Vyhledat a spustit časomíru"
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

        {open && (debounced.length > 0 || filteredFavorites.length > 0) && (
          <SearchDropdown
            results={results}
            favoriteKeys={favoriteKeys}
            highlight={highlight}
            onPick={handlePick}
            onHover={setHighlight}
            loading={q.isFetching && results.length === 0 && debounced.length > 0}
            showingOnlyFavorites={debounced.length === 0}
          />
        )}
      </div>

      {/* Phase 18B — Item 6: comment input between search and clock. */}
      <div className="relative w-44 shrink-0">
        <MessageSquare
          className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--text-tertiary)]"
          aria-hidden
        />
        <input
          type="text"
          value={comment}
          onChange={(e) => setComment(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              if (results.length > 0) {
                onSubmit();
              } else if (onStartUnassigned) {
                onStartBlank();
              }
            }
          }}
          placeholder="Poznámka (volitelné)"
          aria-label="Poznámka k zapnuté časomíře"
          className="w-full h-11 pl-8 pr-3 rounded-[var(--radius-md)]
                     bg-[var(--bg-surface)] border border-[var(--border-subtle)]
                     text-xs text-[var(--text-primary)]
                     placeholder:text-[var(--text-tertiary)]
                     focus:outline-none focus:border-[var(--border-default)]
                     transition-colors duration-150"
        />
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
        aria-label="Spustit časomíru pro vybraný úkol"
      >
        <Play className="w-3.5 h-3.5" aria-hidden />
        Start
      </button>
      {onStartUnassigned && (
        <button
          type="button"
          onClick={onStartBlank}
          className="shrink-0 inline-flex items-center justify-center gap-1.5 px-3 h-11
                     rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                     text-xs text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]
                     transition-colors duration-150"
          aria-label="Spustit časomíru bez úkolu (přiřadit později)"
          title="Spustit bez úkolu — můžete přiřadit později"
        >
          ⚠ Bez úkolu
        </button>
      )}
    </div>
  );
}

function SearchDropdown({
  results,
  favoriteKeys,
  highlight,
  onPick,
  onHover,
  loading,
  showingOnlyFavorites,
}: {
  results: import("../../api/types").IssueRow[];
  favoriteKeys: Set<string>;
  highlight: number;
  onPick: (key: string) => void;
  onHover: (idx: number) => void;
  loading: boolean;
  showingOnlyFavorites: boolean;
}) {
  return (
    <div
      role="listbox"
      className="absolute left-0 right-0 top-full mt-1 z-30
                 rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] shadow-[var(--shadow-md)]
                 max-h-[420px] overflow-y-auto"
    >
      {showingOnlyFavorites && results.length > 0 && (
        <div className="px-3 pt-2 pb-1 text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
          ★ Oblíbené
        </div>
      )}
      {loading && (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">
          Vyhledávání…
        </div>
      )}
      {!loading && results.length === 0 && (
        <div className="px-3 py-2 text-xs text-[var(--text-tertiary)]">
          Žádné odpovídající úkoly.
        </div>
      )}
      {results.map((iss, idx) => {
        const isFav = favoriteKeys.has(iss.issue_key);
        return (
          <div
            key={iss.issue_key}
            role="option"
            aria-selected={idx === highlight}
            onMouseEnter={() => onHover(idx)}
            className={clsx(
              "w-full flex items-center gap-2 px-3 py-2 text-xs",
              idx === highlight
                ? "bg-[var(--bg-hover)] text-[var(--text-primary)]"
                : "text-[var(--text-secondary)]",
            )}
          >
            <FavoriteStar issueKey={iss.issue_key} initial={isFav} size={12} />
            <button
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                onPick(iss.issue_key);
              }}
              className="flex-1 min-w-0 text-left flex items-center gap-2"
            >
              <span className="font-mono uppercase text-[11px] text-[var(--text-tertiary)] w-20 shrink-0">
                {iss.issue_key}
              </span>
              <span className="truncate flex-1 text-[var(--text-primary)]">
                {iss.summary || "(načítá se…)"}
              </span>
            </button>
          </div>
        );
      })}
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
  // Phase 18A — Item 4: unassigned timer surfaces ⚠ + red ring.
  const unassigned = !active.issue_key;

  // Phase 18B — Item 6: live editable comment chip.
  const [editingComment, setEditingComment] = useState(false);
  const [draftComment, setDraftComment] = useState(active.comment ?? "");
  const setComment = useTimerStore((s) => s.setComment);

  useEffect(() => {
    if (!editingComment) {
      setDraftComment(active.comment ?? "");
    }
  }, [active.comment, editingComment]);

  const commitComment = async () => {
    setEditingComment(false);
    const next = draftComment.trim();
    const cur = (active.comment ?? "").trim();
    if (next === cur) return;
    await setComment(next.length > 0 ? next : null);
  };

  return (
    <div className="flex items-stretch gap-2">
      <div
        className={clsx(
          "flex-1 min-w-0 relative h-11 rounded-[var(--radius-md)]",
          "bg-[var(--bg-surface)] flex items-center px-4 gap-3",
          unassigned
            ? "border-2 border-red-500/70"
            : "border border-[var(--border-subtle)]",
        )}
      >
        <span
          aria-hidden
          className={clsx(
            "w-2 h-2 rounded-full animate-pulse shrink-0",
            unassigned ? "bg-red-500" : "bg-[var(--accent)]",
          )}
        />
        <span
          className={clsx(
            "font-mono text-[11px] uppercase tracking-[0.08em] shrink-0",
            unassigned ? "text-red-500" : "text-[var(--accent)]",
          )}
        >
          {unassigned ? "⚠ BEZ ÚKOLU" : active.issue_key}
        </span>
        {editingComment ? (
          <input
            type="text"
            autoFocus
            value={draftComment}
            onChange={(e) => setDraftComment(e.target.value)}
            onBlur={commitComment}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") {
                setDraftComment(active.comment ?? "");
                setEditingComment(false);
              }
            }}
            placeholder="Poznámka"
            aria-label="Upravit poznámku"
            className="flex-1 min-w-0 h-7 px-2 text-xs rounded-[var(--radius-sm)]
                       bg-transparent border border-[var(--border-subtle)]
                       focus:outline-none focus:border-[var(--border-default)]"
          />
        ) : (
          <button
            type="button"
            onClick={() => setEditingComment(true)}
            className="flex-1 min-w-0 text-left text-xs text-[var(--text-tertiary)] truncate
                       hover:text-[var(--text-secondary)] transition-colors duration-150"
            title="Upravit poznámku"
          >
            {active.comment && active.comment.trim().length > 0 ? (
              <span className="inline-flex items-center gap-1.5">
                <MessageSquare className="w-3 h-3 shrink-0" aria-hidden />
                <span className="truncate">{active.comment}</span>
              </span>
            ) : unassigned ? (
              "Přiřaďte úkol před uložením"
            ) : (
              <span className="text-[var(--text-tertiary)]">+ poznámka</span>
            )}
          </button>
        )}
        <span
          className={clsx(
            "ml-auto font-mono tabular-nums text-sm",
            unassigned ? "text-red-500" : "text-[var(--accent)]",
          )}
        >
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

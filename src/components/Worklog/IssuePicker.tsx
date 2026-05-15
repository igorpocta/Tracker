/**
 * Inline issue search-and-pick — used on worklog rows whose `issue_key` is
 * empty (the user stopped the timer without an issue assigned, or created
 * a manual entry with no issue).
 *
 * Click the "Přiřadit úkol" button → small popover opens with a search
 * input + result list. Pick an issue, parent calls `onPick(issue_key)`,
 * popover closes.
 *
 * The popover uses the same `search_issues_cache` backend as the global
 * StartTrackingBar, so behaviour is consistent.
 */
import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { getSuggestedIssues, searchIssuesCache } from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";

const LIMIT = 12;
const DEBOUNCE_MS = 150;

export interface IssuePickerProps {
  onPick: (issueKey: string) => Promise<void> | void;
  disabled?: boolean;
}

export function IssuePicker({ onPick, disabled = false }: IssuePickerProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [busy, setBusy] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(t);
  }, [query]);

  // Empty input → recently-tracked-on issues. Non-empty → full search.
  const searchQ = useQuery({
    queryKey: ["picker-search", debounced, LIMIT],
    queryFn: () => searchIssuesCache(debounced, LIMIT),
    enabled: open && debounced.length > 0,
  });
  const recentQ = useQuery({
    queryKey: queryKeys.suggestedIssues.list(LIMIT),
    queryFn: () => getSuggestedIssues(LIMIT),
    enabled: open && debounced.length === 0,
    staleTime: 30_000,
  });

  const results =
    debounced.length > 0 ? (searchQ.data ?? []) : (recentQ.data ?? []);

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    function onClick(e: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [open]);

  // Close on Escape.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const handlePick = async (key: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await onPick(key);
      setOpen(false);
      setQuery("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div ref={containerRef} className="relative shrink-0">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        disabled={disabled || busy}
        title="Přiřadit úkol k záznamu"
        className="inline-flex items-center gap-1 px-2 h-6 rounded-full
                   font-mono text-[10px] uppercase tracking-[0.08em]
                   border border-dashed text-[var(--text-tertiary)]
                   hover:text-[var(--accent)] hover:border-[var(--accent)]
                   disabled:opacity-50 disabled:cursor-not-allowed
                   transition-colors duration-150"
        style={{ borderColor: "var(--border-default)" }}
      >
        <Plus className="w-3 h-3" aria-hidden />
        Přiřadit úkol
      </button>

      {open && (
        <div
          role="listbox"
          className="absolute left-0 top-full mt-1 z-30 w-80
                     rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                     bg-[var(--bg-surface)] shadow-[var(--shadow-md)]
                     max-h-[360px] overflow-y-auto"
        >
          <div className="p-2 border-b border-[var(--border-subtle)]">
            <input
              type="text"
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Hledat úkol…"
              className="w-full h-8 px-2 rounded-[var(--radius-sm)]
                         bg-transparent border border-[var(--border-subtle)]
                         text-xs text-[var(--text-primary)]
                         focus:outline-none focus:border-[var(--border-default)]"
            />
          </div>
          {debounced.length === 0 && results.length > 0 && (
            <div className="px-3 pt-2 pb-1 text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
              Naposledy trackováno
            </div>
          )}
          {results.length === 0 && (
            <div className="px-3 py-3 text-xs text-[var(--text-tertiary)]">
              {debounced.length === 0
                ? "Začni psát pro vyhledání úkolu."
                : "Žádné odpovídající úkoly."}
            </div>
          )}
          {results.map((iss) => (
            <button
              key={iss.issue_key}
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                void handlePick(iss.issue_key);
              }}
              className="w-full text-left flex items-center gap-2 px-3 py-1.5 text-xs
                         hover:bg-[var(--bg-hover)]"
            >
              <span className="font-mono uppercase text-[11px] text-[var(--text-tertiary)] w-24 shrink-0">
                {iss.issue_key}
              </span>
              <span className="truncate flex-1 text-[var(--text-primary)]">
                {iss.summary || "(bez názvu)"}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

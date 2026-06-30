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
import { Plus } from "lucide-react";
import type { ReactNode } from "react";
import { useCallback, useRef, useState } from "react";

import { useClickOutside } from "../../hooks/useClickOutside";
import { useEscapeKey } from "../../hooks/useEscapeKey";
import { useIssueSearch } from "../../hooks/useIssueSearch";

const LIMIT = 12;

export interface IssuePickerTriggerArgs {
  /** Whether the popover is currently open. */
  open: boolean;
  /** Toggle the popover open/closed. */
  toggle: () => void;
  /** True while an `onPick` call is in-flight. */
  busy: boolean;
}

export interface IssuePickerProps {
  onPick: (issueKey: string) => Promise<void> | void;
  disabled?: boolean;
  /**
   * Custom trigger renderer. When omitted, the default dashed
   * "Přiřadit úkol" button is used (worklog-row variant). Supply this to
   * reuse the picker behind a different control — e.g. the running-timer
   * issue chip in `StartTrackingBar`.
   */
  renderTrigger?: (args: IssuePickerTriggerArgs) => ReactNode;
  /** Wrapper class — defaults to `relative shrink-0`. */
  className?: string;
}

export function IssuePicker({
  onPick,
  disabled = false,
  renderTrigger,
  className = "relative shrink-0",
}: IssuePickerProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const { query, setQuery, debounced, results } = useIssueSearch({
    enabled: open,
    limit: LIMIT,
  });

  const closePopover = useCallback(() => setOpen(false), []);
  useClickOutside(containerRef, closePopover, open);
  useEscapeKey(closePopover, open);

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

  const toggle = useCallback(() => setOpen((v) => !v), []);

  return (
    <div ref={containerRef} className={className}>
      {renderTrigger ? (
        renderTrigger({ open, toggle, busy })
      ) : (
        <button
          type="button"
          onClick={toggle}
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
      )}

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

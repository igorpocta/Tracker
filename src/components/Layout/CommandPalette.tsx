/**
 * Cmd/Ctrl+K command palette.
 *
 * Lets the user jump between routes or pick an issue from the cache. Pretty
 * minimal — no fuzzy scoring, no recent commands history — but enough to make
 * the app feel keyboard-driven.
 */
import { useQuery } from "@tanstack/react-query";
import { clsx } from "clsx";
import { BarChart3, CalendarDays, Search, Settings as SettingsIcon, Sun } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { searchIssuesCache } from "../../api/commands";
import type { IssueRow } from "../../api/types";

export interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  /** Optional handler the parent can wire to start a timer for the picked issue. */
  onStartIssue?: (issueKey: string) => void;
}

interface RouteOption {
  kind: "route";
  to: string;
  label: string;
  icon: React.ReactNode;
}
interface IssueOption {
  kind: "issue";
  issue: IssueRow;
}
type Option = RouteOption | IssueOption;

const ROUTE_OPTIONS: RouteOption[] = [
  { kind: "route", to: "/", label: "Today", icon: <Sun className="w-3.5 h-3.5" aria-hidden /> },
  { kind: "route", to: "/history", label: "History", icon: <CalendarDays className="w-3.5 h-3.5" aria-hidden /> },
  { kind: "route", to: "/reports", label: "Reports", icon: <BarChart3 className="w-3.5 h-3.5" aria-hidden /> },
  { kind: "route", to: "/settings", label: "Settings", icon: <SettingsIcon className="w-3.5 h-3.5" aria-hidden /> },
];

export function CommandPalette({
  open,
  onClose,
  onStartIssue,
}: CommandPaletteProps) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [highlight, setHighlight] = useState(0);

  useEffect(() => {
    if (open) {
      setQuery("");
      setDebounced("");
      setHighlight(0);
      // Defer focus so the input is mounted by the time we focus it.
      window.setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  // Debounce the search input by 120ms — keeps SQLite query frequency sane.
  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), 120);
    return () => window.clearTimeout(t);
  }, [query]);

  const issuesQ = useQuery({
    queryKey: ["palette-issues", debounced],
    queryFn: () => searchIssuesCache(debounced, 10),
    enabled: open && debounced.length > 0,
  });

  const routeMatches = ROUTE_OPTIONS.filter((r) =>
    debounced.length === 0 ? true : r.label.toLowerCase().includes(debounced.toLowerCase()),
  );

  const options: Option[] = [
    ...routeMatches.map<Option>((r) => r),
    ...(issuesQ.data ?? []).map<Option>((i) => ({ kind: "issue", issue: i })),
  ];

  const choose = (opt: Option) => {
    if (opt.kind === "route") {
      navigate(opt.to);
    } else if (onStartIssue) {
      onStartIssue(opt.issue.issue_key);
    }
    onClose();
  };

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-start justify-center pt-24 p-6"
    >
      <div className="w-full max-w-xl bg-neutral-900 border border-neutral-800 rounded-xl shadow-2xl overflow-hidden">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-neutral-800">
          <Search className="w-4 h-4 text-neutral-500" aria-hidden />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setHighlight(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                onClose();
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setHighlight((h) => Math.min(h + 1, Math.max(0, options.length - 1)));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setHighlight((h) => Math.max(h - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                const opt = options[highlight];
                if (opt) choose(opt);
              }
            }}
            placeholder="Search issues or jump to a view…"
            aria-label="Command palette search"
            className="flex-1 bg-transparent text-sm outline-none placeholder:text-neutral-500"
          />
        </div>

        <ul className="max-h-72 overflow-y-auto p-1" role="listbox">
          {options.length === 0 ? (
            <li className="px-3 py-4 text-xs text-neutral-500 text-center">
              No matches.
            </li>
          ) : (
            options.map((opt, idx) => (
              <li key={optionKey(opt)}>
                <button
                  type="button"
                  role="option"
                  aria-selected={idx === highlight}
                  onMouseEnter={() => setHighlight(idx)}
                  onClick={() => choose(opt)}
                  className={clsx(
                    "w-full text-left px-2.5 py-1.5 rounded-md flex items-center gap-2 text-xs",
                    idx === highlight
                      ? "bg-sky-600/20 text-white"
                      : "text-neutral-200 hover:bg-neutral-800/60",
                  )}
                >
                  {opt.kind === "route" ? (
                    <>
                      <span className="text-neutral-400">{opt.icon}</span>
                      <span className="font-medium">Go to {opt.label}</span>
                    </>
                  ) : (
                    <>
                      <span className="font-mono text-[11px] text-neutral-400 w-16 shrink-0">
                        {opt.issue.issue_key}
                      </span>
                      <span className="truncate flex-1">
                        {opt.issue.summary || "(no summary)"}
                      </span>
                      <span className="text-[10px] text-neutral-500">
                        {onStartIssue ? "↵ start timer" : "↵ open"}
                      </span>
                    </>
                  )}
                </button>
              </li>
            ))
          )}
        </ul>

        <div className="px-3 py-1.5 border-t border-neutral-800 flex items-center gap-3 text-[10px] text-neutral-500">
          <kbd className="font-mono">↑↓</kbd> navigate
          <kbd className="font-mono">↵</kbd> select
          <kbd className="font-mono">Esc</kbd> close
        </div>
      </div>
    </div>
  );
}

function optionKey(opt: Option): string {
  return opt.kind === "route" ? `r:${opt.to}` : `i:${opt.issue.issue_key}`;
}

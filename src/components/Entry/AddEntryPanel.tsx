/**
 * "Add entry" right-side slide-in panel.
 *
 * Reference: `screens/SCR-20260514-rjpx-2.png`.
 *
 *   ┌────────────────────┐
 *   │ ➕ Add entry       │
 *   │ Log time you've…   │
 *   ├────────────────────┤
 *   │ Ticket *           │
 *   │ ┌────────────────┐ │
 *   │ │ Type to search │ │
 *   │ └────────────────┘ │
 *   │                    │
 *   │ Date               │
 *   │ ┌────────────────┐ │
 *   │ │ 14/05/2026     │ │
 *   │ └────────────────┘ │
 *   │                    │
 *   │ Start & end time   │
 *   │ [15m][30m][1h]…    │
 *   │ [19:57] → [19:57]  │
 *   │                    │
 *   │ Comment (optional) │
 *   │ ┌────────────────┐ │
 *   │ │                │ │
 *   │ └────────────────┘ │
 *   ├────────────────────┤
 *   │ -- Total duration  │
 *   │  [ Save entry ]    │
 *   └────────────────────┘
 *
 * Currently a UI-only shell — when "Save entry" is pressed we close the
 * panel and emit a toast. A full Phase-13B implementation would dispatch
 * a backend command to create a manual worklog row; that command is out
 * of scope for this redesign pass (the UI surface is the deliverable).
 */
import { useQuery } from "@tanstack/react-query";
import { Plus, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { searchIssuesCache } from "../../api/commands";
import type { IssueRow } from "../../api/types";
import { Button } from "../common/Button";

export interface AddEntryPanelProps {
  open: boolean;
  onClose: () => void;
  /** Called with the resolved values when the user presses "Save entry". */
  onSave?: (entry: {
    issueKey: string;
    dateIso: string;
    startTime: string;
    endTime: string;
    comment: string;
  }) => Promise<void> | void;
}

const QUICK_DURATIONS = [
  { label: "15m", minutes: 15 },
  { label: "30m", minutes: 30 },
  { label: "1h", minutes: 60 },
  { label: "2h", minutes: 120 },
  { label: "4h", minutes: 240 },
  { label: "8h", minutes: 480 },
];

export function AddEntryPanel({ open, onClose, onSave }: AddEntryPanelProps) {
  const today = useMemo(() => formatLocalDate(new Date()), []);
  const nowHHMM = useMemo(() => formatLocalTime(new Date()), []);

  const [issueQuery, setIssueQuery] = useState("");
  const [issueKey, setIssueKey] = useState("");
  const [issuePickerOpen, setIssuePickerOpen] = useState(false);
  const [dateIso, setDateIso] = useState(today);
  const [start, setStart] = useState(nowHHMM);
  const [end, setEnd] = useState(nowHHMM);
  const [comment, setComment] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const issueContainerRef = useRef<HTMLDivElement | null>(null);

  // Reset state every time the panel opens.
  useEffect(() => {
    if (!open) return;
    setIssueQuery("");
    setIssueKey("");
    setDateIso(today);
    setStart(nowHHMM);
    setEnd(nowHHMM);
    setComment("");
    setError(null);
  }, [open, today, nowHHMM]);

  const debounced = useDebounced(issueQuery, 120);
  const issuesQ = useQuery({
    queryKey: ["search-issues", debounced, 12],
    queryFn: () => searchIssuesCache(debounced, 12),
    enabled: open && debounced.length > 0,
  });
  const issueResults = issuesQ.data ?? [];

  // Close the issue picker on outside click.
  useEffect(() => {
    if (!issuePickerOpen) return;
    function onClick(e: MouseEvent) {
      if (
        issueContainerRef.current &&
        !issueContainerRef.current.contains(e.target as Node)
      ) {
        setIssuePickerOpen(false);
      }
    }
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [issuePickerOpen]);

  if (!open) return null;

  const totalMinutes = computeDurationMinutes(start, end);
  const totalLabel =
    totalMinutes <= 0
      ? "—"
      : totalMinutes < 60
        ? `${totalMinutes}m`
        : totalMinutes % 60 === 0
          ? `${Math.floor(totalMinutes / 60)}h`
          : `${Math.floor(totalMinutes / 60)}h ${totalMinutes % 60}m`;

  const handleDurationClick = (minutes: number) => {
    setEnd(addMinutes(start, minutes));
  };

  const handleSubmit = async () => {
    setError(null);
    if (!issueKey) {
      setError("Nejprve vyberte úkol.");
      return;
    }
    if (totalMinutes <= 0) {
      setError("Konec musí být po začátku.");
      return;
    }
    setSaving(true);
    try {
      await onSave?.({
        issueKey,
        dateIso,
        startTime: start,
        endTime: end,
        comment: comment.trim(),
      });
      onClose();
    } catch (e) {
      setError(typeof e === "string" ? e : "Záznam se nepodařilo uložit");
    } finally {
      setSaving(false);
    }
  };

  return (
    <aside
      role="dialog"
      aria-label="Přidat záznam"
      className="w-[300px] shrink-0 h-full overflow-y-auto
                 border-l border-[var(--border-subtle)]
                 bg-[var(--bg-surface)]
                 flex flex-col"
    >
      <header className="flex items-start gap-3 p-4 border-b border-[var(--border-subtle)]">
        <span
          aria-hidden
          className="w-8 h-8 rounded-lg flex items-center justify-center"
          style={{ background: "var(--accent-soft)", color: "var(--accent)" }}
        >
          <Plus className="w-4 h-4" />
        </span>
        <div className="flex-1 min-w-0">
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">
            Přidat záznam
          </h2>
          <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5">
            Zaznamenat strávený čas
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Zavřít panel přidání záznamu"
          className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                     transition-colors duration-150"
        >
          <X className="w-4 h-4" />
        </button>
      </header>

      <div className="flex-1 p-4 flex flex-col gap-4">
        {/* Ticket */}
        <div ref={issueContainerRef} className="relative">
          <FieldLabel required>Úkol</FieldLabel>
          <input
            type="text"
            value={issueKey || issueQuery}
            onChange={(e) => {
              setIssueKey("");
              setIssueQuery(e.target.value);
              setIssuePickerOpen(true);
            }}
            onFocus={() => setIssuePickerOpen(issueQuery.length > 0)}
            placeholder="Vyhledat psaním"
            className={inputCls}
          />
          {issuePickerOpen && issueResults.length > 0 && (
            <IssuePicker
              results={issueResults}
              onPick={(iss) => {
                setIssueKey(iss.issue_key);
                setIssueQuery(`${iss.issue_key} · ${iss.summary ?? ""}`);
                setIssuePickerOpen(false);
              }}
            />
          )}
        </div>

        {/* Date */}
        <div>
          <FieldLabel>Datum</FieldLabel>
          <input
            type="date"
            value={dateIso}
            onChange={(e) => setDateIso(e.target.value)}
            className={inputCls}
          />
        </div>

        {/* Start / end time */}
        <div>
          <FieldLabel>Začátek a konec</FieldLabel>
          <div className="flex flex-wrap gap-1 mb-2">
            {QUICK_DURATIONS.map((d) => (
              <button
                key={d.label}
                type="button"
                onClick={() => handleDurationClick(d.minutes)}
                className="px-2 h-6 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                           text-[11px] text-[var(--text-secondary)]
                           hover:bg-[var(--bg-hover)] transition-colors duration-150"
              >
                {d.label}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-1.5">
            <input
              type="time"
              value={start}
              onChange={(e) => setStart(e.target.value)}
              className={`${inputCls} flex-1`}
              aria-label="Začátek"
            />
            <span aria-hidden className="text-[var(--text-tertiary)]">→</span>
            <input
              type="time"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
              className={`${inputCls} flex-1`}
              aria-label="Konec"
            />
          </div>
        </div>

        {/* Comment */}
        <div>
          <FieldLabel>Komentář (volitelné)</FieldLabel>
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            rows={4}
            className={`${inputCls} resize-none`}
          />
        </div>

        {error && (
          <div className="text-xs text-[var(--danger)]" role="alert">
            {error}
          </div>
        )}
      </div>

      <footer className="p-4 border-t border-[var(--border-subtle)]
                         flex items-center justify-between gap-3">
        <div className="text-[11px] text-[var(--text-tertiary)]">
          <div className="font-mono tabular-nums text-[var(--text-primary)] text-sm">
            {totalLabel}
          </div>
          Celkem
        </div>
        <Button
          variant="primary"
          size="md"
          onClick={handleSubmit}
          disabled={saving || !issueKey || totalMinutes <= 0}
        >
          Uložit záznam
        </Button>
      </footer>
    </aside>
  );
}

function IssuePicker({
  results,
  onPick,
}: {
  results: IssueRow[];
  onPick: (iss: IssueRow) => void;
}) {
  return (
    <div
      role="listbox"
      className="absolute left-0 right-0 top-full mt-1 z-30
                 max-h-64 overflow-y-auto
                 rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                 bg-[var(--bg-surface)] shadow-[var(--shadow-md)]"
    >
      {results.map((iss) => (
        <button
          key={iss.issue_key}
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            onPick(iss);
          }}
          className="w-full text-left flex items-center gap-2 px-3 py-2 text-xs
                     hover:bg-[var(--bg-hover)] transition-colors duration-150"
        >
          <span className="font-mono text-[10px] uppercase text-[var(--accent)] w-16 shrink-0">
            {iss.issue_key}
          </span>
          <span className="truncate text-[var(--text-primary)]">
            {iss.summary || "(načítá se…)"}
          </span>
        </button>
      ))}
    </div>
  );
}

function FieldLabel({
  children,
  required = false,
}: {
  children: React.ReactNode;
  required?: boolean;
}) {
  return (
    <label className="block text-[11px] font-medium text-[var(--text-secondary)] mb-1.5">
      {children}
      {required && <span className="text-[var(--accent)] ml-0.5">*</span>}
    </label>
  );
}

const inputCls =
  "w-full h-9 px-3 rounded-[var(--radius-md)] " +
  "bg-transparent border border-[var(--border-subtle)] " +
  "text-sm text-[var(--text-primary)] " +
  "placeholder:text-[var(--text-tertiary)] " +
  "focus:outline-none focus:border-[var(--border-default)] " +
  "transition-colors duration-150";

function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(value), ms);
    return () => window.clearTimeout(t);
  }, [value, ms]);
  return debounced;
}

function formatLocalDate(d: Date): string {
  const yyyy = d.getFullYear();
  const mm = `${d.getMonth() + 1}`.padStart(2, "0");
  const dd = `${d.getDate()}`.padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

function formatLocalTime(d: Date): string {
  return `${`${d.getHours()}`.padStart(2, "0")}:${`${d.getMinutes()}`.padStart(2, "0")}`;
}

/** Returns total minutes between two `HH:MM` strings. Handles end < start as 0. */
export function computeDurationMinutes(start: string, end: string): number {
  const a = parseHHMM(start);
  const b = parseHHMM(end);
  if (a === null || b === null) return 0;
  const diff = b - a;
  return diff > 0 ? diff : 0;
}

function parseHHMM(s: string): number | null {
  const m = /^(\d{1,2}):(\d{1,2})$/.exec(s);
  if (!m) return null;
  const h = parseInt(m[1], 10);
  const mm = parseInt(m[2], 10);
  if (h < 0 || h > 23 || mm < 0 || mm > 59) return null;
  return h * 60 + mm;
}

function addMinutes(start: string, mins: number): string {
  const a = parseHHMM(start);
  if (a === null) return start;
  const total = a + mins;
  const h = Math.floor(total / 60) % 24;
  const m = total % 60;
  return `${`${h}`.padStart(2, "0")}:${`${m}`.padStart(2, "0")}`;
}

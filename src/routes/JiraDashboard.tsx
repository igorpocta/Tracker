/**
 * "JIRA Přehled" — cross-connection tabulka úkolů.
 *
 * Backend (`get_jira_dashboard_issues`) projde všechny Jira connections s
 * `dashboard_enabled = true`, spustí jejich `dashboard_jql` a vrátí flat
 * řádky. Tato komponenta je jenom tabulka se sortovatelnými hlavičkami a
 * avatary u osob.
 */
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, ArrowDown, ArrowUp, RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";

import {
  getJiraDashboardIssues,
  openJiraIssue,
  type JiraDashboardPerson,
  type JiraDashboardRow,
} from "../api/commands";

type SortKey =
  | "issue_key"
  | "assignee"
  | "reporter"
  | "priority"
  | "status"
  | "created"
  | "due_date";
type SortDir = "asc" | "desc";

interface SortState {
  key: SortKey;
  dir: SortDir;
}

const COLUMNS: { key: SortKey; label: string; align?: "left" | "right" }[] = [
  { key: "issue_key", label: "Úkol" },
  { key: "assignee", label: "Pověřená osoba" },
  { key: "reporter", label: "Zadavatel" },
  { key: "priority", label: "Priorita" },
  { key: "status", label: "Stav" },
  { key: "created", label: "Vytvořeno" },
  { key: "due_date", label: "Termín dokončení" },
];

export default function JiraDashboard() {
  const [sort, setSort] = useState<SortState>({ key: "issue_key", dir: "asc" });

  const q = useQuery({
    queryKey: ["jira-dashboard"],
    queryFn: getJiraDashboardIssues,
    staleTime: 60_000,
  });

  const rows = q.data?.rows ?? [];
  const errors = q.data?.errors ?? [];

  const sortedRows = useMemo(() => sortRows(rows, sort), [rows, sort]);

  const toggle = (key: SortKey) => {
    setSort((cur) =>
      cur.key === key
        ? { key, dir: cur.dir === "asc" ? "desc" : "asc" }
        : { key, dir: "asc" },
    );
  };

  return (
    <div className="px-6 pb-6 pt-2 flex flex-col gap-5 w-full max-w-[1400px] mx-auto">
      <header className="flex items-baseline justify-between gap-4 flex-wrap pt-2">
        <div className="flex items-baseline gap-3 flex-wrap">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            JIRA Přehled
          </h1>
          <span className="text-xs text-[var(--text-tertiary)]">
            {q.isLoading
              ? "Načítám…"
              : `${rows.length} úkol${rows.length === 1 ? "" : "ů"}`}
          </span>
        </div>
        <button
          type="button"
          onClick={() => q.refetch()}
          disabled={q.isFetching}
          className="inline-flex items-center gap-1.5 px-3 h-8
                     rounded-[var(--radius-md)] text-xs text-[var(--accent)]
                     border border-[var(--accent-soft)]
                     bg-transparent hover:bg-[var(--accent-soft)]
                     transition-colors duration-150
                     disabled:opacity-60 disabled:cursor-progress"
          title="Aktualizovat z Jiry"
        >
          <RefreshCw
            className={`w-3.5 h-3.5 ${q.isFetching ? "animate-spin" : ""}`}
            aria-hidden
          />
          Aktualizovat
        </button>
      </header>

      {errors.length > 0 && (
        <div className="rounded-[var(--radius-md)] border border-[color-mix(in_srgb,var(--danger)_30%,transparent)] p-3 flex flex-col gap-1">
          {errors.map((e) => (
            <div
              key={e.connection_id}
              className="flex items-start gap-2 text-xs text-[var(--text-secondary)]"
            >
              <AlertTriangle
                className="w-3.5 h-3.5 mt-0.5 shrink-0 text-[var(--danger)]"
                aria-hidden
              />
              <span>
                <span className="font-medium text-[var(--text-primary)]">
                  {e.connection_name}
                </span>
                : {e.error}
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="rounded-[var(--radius-lg)] border border-[var(--border-subtle)] bg-[var(--bg-surface)] overflow-x-auto">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr
              className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]"
              style={{
                borderBottom: "1px solid var(--border-subtle)",
              }}
            >
              {COLUMNS.map((col) => (
                <HeaderCell
                  key={col.key}
                  label={col.label}
                  active={sort.key === col.key}
                  dir={sort.dir}
                  onClick={() => toggle(col.key)}
                />
              ))}
            </tr>
          </thead>
          <tbody>
            {sortedRows.length === 0 && !q.isLoading && (
              <tr>
                <td
                  colSpan={COLUMNS.length}
                  className="py-8 text-center text-[var(--text-tertiary)]"
                >
                  Žádné úkoly nevyhovují JQL filtru.
                </td>
              </tr>
            )}
            {sortedRows.map((r) => (
              <DashboardRowView key={`${r.connection_id}-${r.issue_key}`} row={r} />
            ))}
          </tbody>
        </table>
      </div>

      {rows.length === 0 && !q.isLoading && errors.length === 0 && (
        <p className="text-xs text-[var(--text-tertiary)] text-center">
          Žádná Jira integrace zatím nemá zapnutý Dashboard. Zapni ho v
          Nastavení → Připojení → Upravit.
        </p>
      )}
    </div>
  );
}

function HeaderCell({
  label,
  active,
  dir,
  onClick,
}: {
  label: string;
  active: boolean;
  dir: SortDir;
  onClick: () => void;
}) {
  return (
    <th
      onClick={onClick}
      className="px-3 py-2 text-left font-medium select-none cursor-pointer
                 hover:text-[var(--text-primary)] transition-colors duration-150"
    >
      <span className="inline-flex items-center gap-1">
        {label}
        {active &&
          (dir === "asc" ? (
            <ArrowUp className="w-3 h-3" aria-hidden />
          ) : (
            <ArrowDown className="w-3 h-3" aria-hidden />
          ))}
      </span>
    </th>
  );
}

function DashboardRowView({ row }: { row: JiraDashboardRow }) {
  const onIssueClick = (e: React.MouseEvent) => {
    e.preventDefault();
    void openJiraIssue(row.issue_key);
  };
  return (
    <tr
      className="hover:bg-[var(--accent-soft)] transition-colors duration-100"
      style={{ borderBottom: "1px solid var(--border-subtle)" }}
    >
      <td className="px-3 py-2 align-middle">
        <button
          type="button"
          onClick={onIssueClick}
          className="font-mono font-semibold text-[var(--accent)] hover:underline"
          title={`Otevřít ${row.issue_key} v Jiře`}
        >
          {row.issue_key}
        </button>
        <div className="text-[var(--text-secondary)] truncate max-w-[420px]">
          {row.summary}
        </div>
      </td>
      <td className="px-3 py-2 align-middle">
        <PersonCell person={row.assignee} />
      </td>
      <td className="px-3 py-2 align-middle">
        <PersonCell person={row.reporter} />
      </td>
      <td className="px-3 py-2 align-middle">
        <PriorityCell priority={row.priority} />
      </td>
      <td className="px-3 py-2 align-middle">
        <StatusCell status={row.status} category={row.status_category} />
      </td>
      <td className="px-3 py-2 align-middle font-mono tabular-nums text-[var(--text-tertiary)]">
        {formatDate(row.created)}
      </td>
      <td className="px-3 py-2 align-middle tabular-nums">
        <DueDate value={row.due_date} />
      </td>
    </tr>
  );
}

function PersonCell({ person }: { person: JiraDashboardPerson | null | undefined }) {
  if (!person) {
    return <span className="text-[var(--text-tertiary)]">—</span>;
  }
  return (
    <div className="flex items-center gap-2 min-w-0">
      <Avatar person={person} />
      <span className="truncate text-[var(--text-primary)]">
        {person.display_name}
      </span>
    </div>
  );
}

function Avatar({ person }: { person: JiraDashboardPerson }) {
  const initial = (person.display_name[0] ?? "?").toUpperCase();
  if (person.avatar_url) {
    return (
      <img
        src={person.avatar_url}
        alt=""
        width={20}
        height={20}
        className="w-5 h-5 rounded-full shrink-0 object-cover"
        // Jira avatar URL vyžaduje autorizovaný request u některých instalací.
        // Pokud načítání selže, padáme zpět na iniciálu.
        onError={(e) => {
          (e.currentTarget as HTMLImageElement).style.display = "none";
        }}
      />
    );
  }
  return (
    <span
      aria-hidden
      className="w-5 h-5 rounded-full shrink-0 inline-flex items-center justify-center
                 text-[10px] font-bold text-white"
      style={{ background: "var(--accent)" }}
    >
      {initial}
    </span>
  );
}

function PriorityCell({ priority }: { priority?: string | null }) {
  if (!priority) return <span className="text-[var(--text-tertiary)]">—</span>;
  return <span className="text-[var(--text-primary)]">{priority}</span>;
}

function StatusCell({
  status,
  category,
}: {
  status?: string | null;
  category?: string | null;
}) {
  if (!status) return <span className="text-[var(--text-tertiary)]">—</span>;
  const color = statusCategoryColor(category);
  return (
    <span
      className="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium uppercase tracking-wide"
      style={{
        background: color.bg,
        color: color.fg,
      }}
    >
      {status}
    </span>
  );
}

function DueDate({ value }: { value?: string | null }) {
  if (!value) {
    return <span className="text-[var(--text-tertiary)]">—</span>;
  }
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = new Date(value);
  const isOverdue = !Number.isNaN(due.getTime()) && due < today;
  return (
    <span
      style={{
        color: isOverdue ? "var(--danger)" : "var(--text-primary)",
      }}
    >
      {formatDate(value)}
    </span>
  );
}

function statusCategoryColor(category?: string | null): {
  bg: string;
  fg: string;
} {
  switch (category) {
    case "new":
      return { bg: "rgba(100, 116, 139, 0.15)", fg: "#475569" };
    case "indeterminate":
      return { bg: "rgba(59, 130, 246, 0.15)", fg: "#1d4ed8" };
    case "done":
      return { bg: "rgba(34, 197, 94, 0.15)", fg: "#15803d" };
    default:
      return {
        bg: "var(--bg-elevated)",
        fg: "var(--text-secondary)",
      };
  }
}

function formatDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.getDate()}. ${d.getMonth() + 1}. ${d.getFullYear()}`;
}

// ---------------------------------------------------------------------------
// Sorting helpers (čisté, ať to jdou unit-testovat odděleně, kdyby bylo třeba)
// ---------------------------------------------------------------------------

function sortRows(rows: JiraDashboardRow[], sort: SortState): JiraDashboardRow[] {
  const sign = sort.dir === "asc" ? 1 : -1;
  const cmp = (a: JiraDashboardRow, b: JiraDashboardRow) =>
    sign * compareBy(a, b, sort.key);
  return [...rows].sort(cmp);
}

function compareBy(
  a: JiraDashboardRow,
  b: JiraDashboardRow,
  key: SortKey,
): number {
  const av = sortableValue(a, key);
  const bv = sortableValue(b, key);
  if (av == null && bv == null) return 0;
  if (av == null) return 1; // nulls last (asc); flipped by sign for desc
  if (bv == null) return -1;
  if (typeof av === "number" && typeof bv === "number") return av - bv;
  return String(av).localeCompare(String(bv), "cs", { sensitivity: "base" });
}

function sortableValue(
  r: JiraDashboardRow,
  key: SortKey,
): string | number | null {
  switch (key) {
    case "issue_key":
      return r.issue_key;
    case "assignee":
      return r.assignee?.display_name ?? null;
    case "reporter":
      return r.reporter?.display_name ?? null;
    case "priority":
      return r.priority ?? null;
    case "status":
      return r.status ?? null;
    case "created":
      return r.created ? new Date(r.created).getTime() : null;
    case "due_date":
      return r.due_date ? new Date(r.due_date).getTime() : null;
  }
}

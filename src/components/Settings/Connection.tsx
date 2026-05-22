/**
 * Settings → Připojení.
 *
 * Phase 18F rewrite: single source of truth is `list_connections`. The legacy
 * `get_current_config` shim is no longer consulted, so the duplicated "Jira"
 * card the user was seeing is gone.
 *
 * Each card surfaces:
 *   - Provider badge (small icon).
 *   - Inline-editable name (Pencil → input → save on blur / Enter / Escape).
 *   - Account info (email) + provider-specific URL/host.
 *   - Action buttons: Edit credentials, Test, Remove.
 *   - For Freelo: an expandable "Vybrané projekty" list.
 *
 * Adding a new connection opens `AddConnectionDialog` (inline modal); the
 * user never leaves the Settings panel.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2,
  CircleAlert,
  DownloadCloud,
  LoaderCircle,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  getConnectionStats,
  getFreeloSelectedProjects,
  getSyncErrors,
  listConnections,
  listFreeloProjects,
  listSyncRuns,
  refreshConnection,
  removeConnection,
  setFreeloSelectedProjects,
  syncFreeloNow,
  updateConnectionApi,
} from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";
import type { ConnectionDto, FreeloProjectDto } from "../../api/types";

import { AddConnectionDialog } from "./AddConnectionDialog";
import { EditConnectionDialog } from "./EditConnectionDialog";

export default function Connection() {
  const queryClient = useQueryClient();
  const connsQ = useQuery({
    queryKey: queryKeys.connections.all(),
    queryFn: listConnections,
  });
  const syncErrorsQ = useQuery({
    queryKey: queryKeys.syncErrors.all(),
    queryFn: getSyncErrors,
    staleTime: 5_000,
  });
  const [addOpen, setAddOpen] = useState(false);
  const [editConn, setEditConn] = useState<ConnectionDto | null>(null);

  const conns = connsQ.data ?? [];
  const errorByConn = new Map(
    (syncErrorsQ.data ?? []).map((e) => [e.connection_id, e]),
  );

  function refresh() {
    queryClient.invalidateQueries({ queryKey: queryKeys.connections.all() });
    queryClient.invalidateQueries({ queryKey: queryKeys.syncErrors.all() });
  }

  return (
    <div className="flex flex-col gap-4 w-full">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Připojení
        </h2>
        <p className="text-xs text-[var(--text-tertiary)] mt-1">
          Připojte jeden nebo více účtů. Můžete je pojmenovat a kdykoli upravit.
        </p>
      </header>

      <div className="flex flex-col gap-2">
        {connsQ.isLoading && (
          <p className="text-xs text-[var(--text-tertiary)]">Načítám…</p>
        )}

        {conns.map((conn) => (
          <ConnectionCard
            key={conn.id}
            conn={conn}
            syncError={errorByConn.get(conn.id) ?? null}
            onChanged={refresh}
            onEdit={() => setEditConn(conn)}
          />
        ))}

        <button
          type="button"
          onClick={() => setAddOpen(true)}
          className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                     border border-dashed border-[var(--border-subtle)]
                     text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]
                     transition-colors duration-150"
          data-testid="add-connection-button"
        >
          <Plus className="w-4 h-4" aria-hidden />
          <span className="text-sm">Přidat nové připojení</span>
        </button>

        {!connsQ.isLoading && conns.length === 0 && (
          <p className="text-xs text-[var(--text-tertiary)] px-3 py-2">
            Žádná připojení nejsou nakonfigurována. Klikněte na „Přidat nové
            připojení“ pro start.
          </p>
        )}
      </div>

      <AddConnectionDialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onSaved={refresh}
      />

      {editConn && (
        <EditConnectionDialog
          open={editConn !== null}
          conn={editConn}
          onClose={() => setEditConn(null)}
          onSaved={refresh}
        />
      )}

      <SyncRunsHistory />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Connection card
// ---------------------------------------------------------------------------

function ConnectionCard({
  conn,
  syncError,
  onChanged,
  onEdit,
}: {
  conn: ConnectionDto;
  syncError: import("../../api/commands").SyncErrorEntry | null;
  onChanged: () => void;
  onEdit: () => void;
}) {
  const statsQ = useQuery({
    queryKey: queryKeys.connectionStats.for(conn.id),
    queryFn: () => getConnectionStats(conn.id),
    // Refetch po sync — invalidace skrz auto-sync-complete v Sidebar dotahuje sem.
    staleTime: 30_000,
  });
  const [expanded, setExpanded] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [testing, setTesting] = useState<
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ok" }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [fullSyncing, setFullSyncing] = useState(false);

  async function handleFullSync() {
    if (fullSyncing) return;
    if (
      !window.confirm(
        `Stáhnout celou historii pro „${conn.name}"?\n\n` +
          `Toto stáhne všechny úkoly a worklogy ~10 let zpět a může chvíli trvat. ` +
          `Pro běžnou aktualizaci stačí tlačítko v levé liště.`,
      )
    ) {
      return;
    }
    setFullSyncing(true);
    try {
      await refreshConnection(conn.id, "full");
    } catch {
      /* error surfaces via the SyncBanner */
    } finally {
      setFullSyncing(false);
    }
  }

  const cfg = (conn.config ?? {}) as Record<string, unknown>;
  const email =
    typeof cfg["email"] === "string" ? (cfg["email"] as string) : null;
  const baseUrl =
    typeof cfg["base_url"] === "string" ? (cfg["base_url"] as string) : null;

  async function handleTest() {
    setTesting({ kind: "loading" });
    try {
      if (!conn.has_token) {
        throw new Error("Chybí uložený token");
      }
      // Smallest backend round-trip we can use to verify the live client is
      // healthy: a no-op `update_connection` re-hydrates the in-memory client
      // and propagates any provider failure as a thrown error.
      await updateConnectionApi({ id: conn.id });
      setTesting({ kind: "ok" });
      window.setTimeout(() => setTesting({ kind: "idle" }), 2500);
    } catch (e) {
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "Test se nezdařil";
      setTesting({ kind: "error", message });
    }
  }

  return (
    <div
      className="flex flex-col gap-2 p-3 rounded-[var(--radius-md)]
                 border border-[var(--border-subtle)] bg-[var(--bg-surface)]"
      data-testid={`connection-card-${conn.id}`}
    >
      <div className="flex items-center gap-3">
        <ProviderAvatar provider={conn.provider} />
        <div className="flex-1 min-w-0">
          {renaming ? (
            <InlineRename
              initial={conn.name}
              onSubmit={async (next) => {
                setRenaming(false);
                if (next === conn.name) return;
                try {
                  await updateConnectionApi({ id: conn.id, name: next });
                  onChanged();
                } catch {
                  /* ignore */
                }
              }}
              onCancel={() => setRenaming(false)}
            />
          ) : (
            <button
              type="button"
              onClick={() => setRenaming(true)}
              data-testid={`conn-name-${conn.id}`}
              className="text-sm font-medium text-[var(--text-primary)]
                         text-left hover:text-[var(--accent)]
                         transition-colors duration-150 truncate block w-full"
              title="Klikněte pro přejmenování"
            >
              {conn.name}
            </button>
          )}
          <div className="text-[11px] text-[var(--text-tertiary)] truncate">
            {providerLabel(conn.provider)}
            {email ? ` · ${email}` : ""}
            {baseUrl ? ` · ${baseUrl}` : ""}
          </div>
          {statsQ.data && (
            <div className="text-[10px] text-[var(--text-tertiary)] truncate font-mono tabular-nums">
              {statsQ.data.worklog_count.toLocaleString("cs-CZ")} worklog
              {worklogPlural(statsQ.data.worklog_count)} ·{" "}
              {statsQ.data.issue_count.toLocaleString("cs-CZ")} úkol
              {issuePlural(statsQ.data.issue_count)}
              {statsQ.data.last_synced_at
                ? ` · sync ${formatSyncTime(statsQ.data.last_synced_at)}`
                : " · nikdy nesyncováno"}
            </div>
          )}
        </div>

        <ActionButton
          onClick={() => setRenaming((r) => !r)}
          ariaLabel="Přejmenovat"
          testId={`conn-rename-${conn.id}`}
        >
          <Pencil className="w-4 h-4" aria-hidden />
        </ActionButton>

        <ActionButton
          onClick={onEdit}
          ariaLabel="Upravit přihlašovací údaje"
          testId={`conn-edit-${conn.id}`}
        >
          <span className="text-[11px] font-medium">Upravit</span>
        </ActionButton>

        <ActionButton
          onClick={() => void handleTest()}
          ariaLabel="Otestovat připojení"
          testId={`conn-test-${conn.id}`}
        >
          {testing.kind === "loading" ? (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          ) : testing.kind === "ok" ? (
            <CheckCircle2 className="w-4 h-4 text-[var(--success)]" aria-hidden />
          ) : testing.kind === "error" ? (
            <CircleAlert className="w-4 h-4 text-[var(--danger)]" aria-hidden />
          ) : (
            <span className="text-[11px] font-medium">Test</span>
          )}
        </ActionButton>

        <ActionButton
          onClick={() => void handleFullSync()}
          ariaLabel="Stáhnout celou historii (úkoly + worklogy ~10 let)"
          testId={`conn-fullsync-${conn.id}`}
        >
          {fullSyncing ? (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          ) : (
            <DownloadCloud className="w-4 h-4" aria-hidden />
          )}
        </ActionButton>

        <ActionButton
          onClick={async () => {
            if (!window.confirm(`Odpojit „${conn.name}"?`)) return;
            try {
              await removeConnection(conn.id);
              onChanged();
            } catch {
              /* ignore */
            }
          }}
          ariaLabel="Odpojit"
          testId={`conn-remove-${conn.id}`}
          danger
        >
          <Trash2 className="w-4 h-4" aria-hidden />
        </ActionButton>
      </div>

      {testing.kind === "error" && (
        <p
          className="text-[11px] text-[var(--danger)] pl-11"
          role="alert"
        >
          {testing.message}
        </p>
      )}

      {syncError &&
        (() => {
          const isSkip = syncError.phase === "worklogs_skipped";
          // Skip = "fáze se nespustila" (warning), error = "fáze padla" (danger).
          // Same panel layout, different palette + headline shape, so the user
          // can tell "missing config" apart from "remote API blew up" at a glance.
          const accent = isSkip
            ? "var(--warning, var(--text-tertiary))"
            : "var(--danger, #c0392b)";
          return (
            <div
              className="flex items-start gap-2 pl-11 pr-2 py-1.5 rounded-[var(--radius-sm)]"
              style={{
                background: `color-mix(in srgb, ${accent} 8%, transparent)`,
                border: `1px solid color-mix(in srgb, ${accent} 25%, transparent)`,
              }}
              role="alert"
            >
              <CircleAlert
                className="w-3.5 h-3.5 mt-0.5 shrink-0"
                style={{ color: accent }}
                aria-hidden
              />
              <div className="flex-1 min-w-0">
                <p
                  className="text-[11px] font-medium"
                  style={{ color: accent }}
                >
                  {isSkip
                    ? `${syncErrorLabel(syncError.phase)} — ${syncError.error}`
                    : `${syncErrorLabel(syncError.phase)} selhala`}
                </p>
                {!isSkip && (
                  <p className="text-[11px] text-[var(--text-secondary)] break-words">
                    {syncError.error}
                  </p>
                )}
                <p className="text-[10px] text-[var(--text-tertiary)] mt-0.5">
                  {formatErrorTime(syncError.at)}
                </p>
              </div>
            </div>
          );
        })()}

      {conn.provider === "freelo" && (
        <>
          <button
            type="button"
            onClick={() => setExpanded((e) => !e)}
            className="self-start text-xs text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                       transition-colors duration-150 pl-11"
          >
            {expanded ? "Skrýt projekty" : "Vybrané projekty"}
          </button>
          {expanded && <FreeloProjectsPanel connectionId={conn.id} />}
        </>
      )}
    </div>
  );
}

function ActionButton({
  onClick,
  ariaLabel,
  testId,
  danger,
  children,
}: {
  onClick: () => void;
  ariaLabel: string;
  testId?: string;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={ariaLabel}
      data-testid={testId}
      className={
        "shrink-0 inline-flex items-center justify-center gap-1 px-2 h-7 rounded-[var(--radius-sm)] " +
        "transition-colors duration-150 " +
        (danger
          ? "text-[var(--text-tertiary)] hover:text-[var(--danger)] hover:bg-[var(--bg-hover)]"
          : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]")
      }
    >
      {children}
    </button>
  );
}

function InlineRename({
  initial,
  onSubmit,
  onCancel,
}: {
  initial: string;
  onSubmit: (next: string) => void | Promise<void>;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);
  return (
    <input
      ref={inputRef}
      type="text"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          const next = value.trim();
          if (next.length === 0) onCancel();
          else void onSubmit(next);
        } else if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      onBlur={() => {
        const next = value.trim();
        if (next.length === 0 || next === initial) onCancel();
        else void onSubmit(next);
      }}
      aria-label="Nový název"
      data-testid="conn-rename-input"
      className="w-full h-7 px-1.5 rounded-[var(--radius-sm)] bg-transparent
                 border border-[var(--border-default)] focus:border-[var(--accent)]
                 focus:outline-none focus:ring-1 focus:ring-[var(--accent-ring)]
                 text-sm text-[var(--text-primary)] transition-colors duration-150"
    />
  );
}

// ---------------------------------------------------------------------------
// Freelo project picker (kept compatible with previous behavior)
// ---------------------------------------------------------------------------

function FreeloProjectsPanel({ connectionId }: { connectionId: number }) {
  const [projects, setProjects] = useState<FreeloProjectDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await listFreeloProjects(connectionId);
        const selected = await getFreeloSelectedProjects(connectionId);
        if (cancelled) return;
        const sel = new Set(selected);
        setProjects(list.map((p) => ({ ...p, selected: sel.has(p.id) })));
      } catch (e) {
        if (cancelled) return;
        setError(
          typeof e === "string"
            ? e
            : e instanceof Error
              ? e.message
              : "Načtení projektů se nezdařilo",
        );
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [connectionId]);

  async function persist(next: FreeloProjectDto[]) {
    setSaving(true);
    try {
      const ids = next.filter((p) => p.selected).map((p) => p.id);
      await setFreeloSelectedProjects(connectionId, ids);
      try {
        await syncFreeloNow(connectionId);
      } catch {
        /* best-effort */
      }
    } finally {
      setSaving(false);
    }
  }

  function toggle(id: number) {
    setProjects((prev) => {
      const next = prev.map((p) =>
        p.id === id ? { ...p, selected: !p.selected } : p,
      );
      void persist(next);
      return next;
    });
  }

  if (loading) {
    return (
      <p className="text-xs text-[var(--text-tertiary)] pl-11">
        Načítám projekty…
      </p>
    );
  }
  if (error) {
    return (
      <p className="text-xs text-[var(--danger)] pl-11" role="alert">
        {error}
      </p>
    );
  }
  if (projects.length === 0) {
    return (
      <p className="text-xs text-[var(--text-tertiary)] pl-11">
        Žádné projekty nenalezeny.
      </p>
    );
  }
  const selectedCount = projects.filter((p) => p.selected).length;
  return (
    <div className="flex flex-col gap-1 pl-11">
      <p className="text-[11px] text-[var(--text-tertiary)]">
        Vybráno {selectedCount} z {projects.length}
      </p>
      <ul
        className="flex flex-col gap-0.5 max-h-[240px] overflow-y-auto"
        data-testid={`freelo-projects-${connectionId}`}
      >
        {projects.map((p) => (
          <li key={p.id}>
            <label className="flex items-center gap-2 px-1 py-1 text-xs
                              text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]
                              rounded-[var(--radius-sm)] cursor-pointer">
              <input
                type="checkbox"
                checked={p.selected}
                onChange={() => toggle(p.id)}
                disabled={saving}
                className="accent-[var(--accent)]"
              />
              <span className="flex-1 truncate">{p.name}</span>
              {p.state !== "active" && (
                <span className="text-[10px] uppercase text-[var(--text-tertiary)]">
                  {p.state}
                </span>
              )}
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SYNC_RUNS_PAGE_SIZE = 30;
const SYNC_RUNS_FETCH_LIMIT = 500;

function SyncRunsHistory() {
  const [expanded, setExpanded] = useState(false);
  const [page, setPage] = useState(1);
  const q = useQuery({
    queryKey: queryKeys.syncRuns.list(SYNC_RUNS_FETCH_LIMIT),
    queryFn: () => listSyncRuns(SYNC_RUNS_FETCH_LIMIT),
    enabled: expanded,
    staleTime: 30_000,
  });
  const runs = q.data ?? [];
  const totalPages = Math.max(1, Math.ceil(runs.length / SYNC_RUNS_PAGE_SIZE));
  const safePage = Math.min(page, totalPages);
  const pageRows = runs.slice(
    (safePage - 1) * SYNC_RUNS_PAGE_SIZE,
    safePage * SYNC_RUNS_PAGE_SIZE,
  );
  const showPagination = runs.length > SYNC_RUNS_PAGE_SIZE;

  return (
    <section className="flex flex-col gap-2 pt-3 border-t border-[var(--border-subtle)]">
      <button
        type="button"
        onClick={() => setExpanded((e) => !e)}
        className="self-start text-xs text-[var(--text-secondary)]
                   hover:text-[var(--text-primary)] transition-colors duration-150"
      >
        {expanded ? "Skrýt historii synchronizací" : "Historie synchronizací"}
      </button>
      {expanded && (
        <>
          <div className="overflow-x-auto">
            <table className="w-full text-[11px] border-collapse">
              <thead>
                <tr
                  className="text-[10px] uppercase tracking-wider text-[var(--text-tertiary)]"
                  style={{ borderBottom: "1px solid var(--border-subtle)" }}
                >
                  <th className="text-left px-2 py-1.5">Kdy</th>
                  <th className="text-left px-2 py-1.5">Připojení</th>
                  <th className="text-left px-2 py-1.5">Režim</th>
                  <th className="text-right px-2 py-1.5">Úkoly</th>
                  <th className="text-right px-2 py-1.5">Worklogy</th>
                  <th className="text-right px-2 py-1.5">Trvání</th>
                  <th className="text-left px-2 py-1.5">Stav</th>
                </tr>
              </thead>
              <tbody>
                {runs.length === 0 && !q.isLoading && (
                  <tr>
                    <td
                      colSpan={7}
                      className="px-2 py-3 text-center text-[var(--text-tertiary)]"
                    >
                      Žádné záznamy.
                    </td>
                  </tr>
                )}
                {pageRows.map((r) => {
                  const durSec = r.finished_at - r.started_at;
                  return (
                    <tr
                      key={r.id}
                      style={{ borderBottom: "1px solid var(--border-subtle)" }}
                    >
                      <td className="px-2 py-1 text-[var(--text-tertiary)] font-mono">
                        {formatSyncTime(r.finished_at)}
                      </td>
                      <td className="px-2 py-1">{r.connection_name ?? "—"}</td>
                      <td className="px-2 py-1 text-[var(--text-tertiary)]">
                        {r.mode === "full" ? "celá historie" : "přírůstky"}
                      </td>
                      <td className="px-2 py-1 text-right font-mono tabular-nums">
                        {r.issues_count}
                      </td>
                      <td className="px-2 py-1 text-right font-mono tabular-nums">
                        {r.worklogs_count}
                      </td>
                      <td className="px-2 py-1 text-right font-mono tabular-nums text-[var(--text-tertiary)]">
                        {durSec}s
                      </td>
                      <td className="px-2 py-1">
                        {r.error_phase ? (
                          <span
                            className="text-[var(--danger)]"
                            title={r.error_message ?? undefined}
                          >
                            ⚠ {r.error_phase}
                          </span>
                        ) : (
                          <span className="text-[var(--success)]">✓</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          {showPagination && (
            <Pagination
              page={safePage}
              totalPages={totalPages}
              onChange={setPage}
            />
          )}
        </>
      )}
    </section>
  );
}

interface PaginationProps {
  page: number;
  totalPages: number;
  onChange: (page: number) => void;
}

function Pagination({ page, totalPages, onChange }: PaginationProps) {
  const prevDisabled = page <= 1;
  const nextDisabled = page >= totalPages;
  return (
    <nav
      aria-label="Stránkování"
      className="flex items-center justify-end gap-2 text-[11px] text-[var(--text-secondary)]"
    >
      <button
        type="button"
        onClick={() => onChange(page - 1)}
        disabled={prevDisabled}
        className="px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                   hover:bg-[var(--bg-hover)] transition-colors duration-150
                   disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
      >
        ← Předchozí
      </button>
      <span className="tabular-nums">
        {page} / {totalPages}
      </span>
      <button
        type="button"
        onClick={() => onChange(page + 1)}
        disabled={nextDisabled}
        className="px-2 h-7 rounded-[var(--radius-sm)] border border-[var(--border-subtle)]
                   hover:bg-[var(--bg-hover)] transition-colors duration-150
                   disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
      >
        Další →
      </button>
    </nav>
  );
}

function providerLabel(p: string): string {
  if (p === "jira") return "Jira";
  if (p === "freelo") return "Freelo";
  return p;
}

/**
 * Avatar pro Connection card. Žádné emoji — kruh v provider-charakteristické
 * barvě s iniciálou. Drží stejnou velikost i pro neznámé providery
 * (fallback `?` v neutrální šedé).
 */
function ProviderAvatar({ provider }: { provider: string }) {
  const spec = providerAvatarSpec(provider);
  return (
    <div
      aria-hidden
      className="shrink-0 w-8 h-8 rounded-full flex items-center justify-center
                 text-[12px] font-bold tracking-tight select-none"
      style={{
        background: spec.background,
        color: spec.color,
      }}
      title={providerLabel(provider)}
    >
      {spec.initial}
    </div>
  );
}

function providerAvatarSpec(p: string): {
  initial: string;
  background: string;
  color: string;
} {
  // Provider-specific accent: Jira modrá, Freelo zelená.
  if (p === "jira") {
    return { initial: "J", background: "#1B6FE5", color: "#ffffff" };
  }
  if (p === "freelo") {
    return { initial: "F", background: "#2CC067", color: "#ffffff" };
  }
  // Toggl / Clockify / cokoli dalšího → neutrální až do doby, než budou
  // mít vlastní brand barvu.
  return {
    initial: (p[0] ?? "?").toUpperCase(),
    background: "var(--bg-elevated)",
    color: "var(--text-secondary)",
  };
}

/** Plurály pro „worklog/worklogy/worklogů". */
function worklogPlural(n: number): string {
  if (n === 1) return "";
  if (n >= 2 && n <= 4) return "y";
  return "ů";
}
function issuePlural(n: number): string {
  if (n === 1) return "";
  if (n >= 2 && n <= 4) return "y";
  return "ů";
}

function formatSyncTime(unixS: number): string {
  if (!unixS) return "";
  const d = new Date(unixS * 1000);
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  if (sameDay) return `dnes ${hh}:${mm}`;
  return `${d.getDate()}. ${d.getMonth() + 1}. ${hh}:${mm}`;
}

function syncErrorLabel(phase: string): string {
  switch (phase) {
    case "connection":
      return "Připojení";
    case "issues":
      return "Načtení úkolů";
    case "worklogs":
      return "Načtení záznamů";
    case "worklogs_skipped":
      return "Worklog sync byl přeskočen";
    default:
      return phase;
  }
}

function formatErrorTime(unixS: number): string {
  if (!unixS) return "";
  const d = new Date(unixS * 1000);
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  if (sameDay) return `dnes ${hh}:${mm}`;
  return `${d.getDate()}. ${d.getMonth() + 1}. ${hh}:${mm}`;
}

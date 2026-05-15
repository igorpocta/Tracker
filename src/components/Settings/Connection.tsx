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
  LoaderCircle,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  getFreeloSelectedProjects,
  listConnections,
  listFreeloProjects,
  removeConnection,
  setFreeloSelectedProjects,
  syncFreeloNow,
  updateConnectionApi,
} from "../../api/commands";
import type { ConnectionDto, FreeloProjectDto } from "../../api/types";

import { AddConnectionDialog } from "./AddConnectionDialog";
import { EditConnectionDialog } from "./EditConnectionDialog";

export default function Connection() {
  const queryClient = useQueryClient();
  const connsQ = useQuery({
    queryKey: ["connections"],
    queryFn: listConnections,
  });
  const [addOpen, setAddOpen] = useState(false);
  const [editConn, setEditConn] = useState<ConnectionDto | null>(null);

  const conns = connsQ.data ?? [];

  function refresh() {
    queryClient.invalidateQueries({ queryKey: ["connections"] });
  }

  return (
    <div className="flex flex-col gap-4 max-w-xl">
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
    </div>
  );
}

// ---------------------------------------------------------------------------
// Connection card
// ---------------------------------------------------------------------------

function ConnectionCard({
  conn,
  onChanged,
  onEdit,
}: {
  conn: ConnectionDto;
  onChanged: () => void;
  onEdit: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [testing, setTesting] = useState<
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ok" }
    | { kind: "error"; message: string }
  >({ kind: "idle" });

  const cfg = (conn.config ?? {}) as Record<string, unknown>;
  const email =
    typeof cfg["email"] === "string" ? (cfg["email"] as string) : null;
  const baseUrl =
    typeof cfg["base_url"] === "string" ? (cfg["base_url"] as string) : null;
  const icon = providerIcon(conn.provider);

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
        <div
          className="w-8 h-8 rounded-full flex items-center justify-center text-base"
          style={{
            background: "var(--accent-soft)",
          }}
          aria-hidden
        >
          {icon}
        </div>
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

function providerLabel(p: string): string {
  if (p === "jira") return "Jira";
  if (p === "freelo") return "Freelo";
  return p;
}

function providerIcon(p: string): string {
  if (p === "jira") return "🔷";
  if (p === "freelo") return "🟢";
  return "🔗";
}

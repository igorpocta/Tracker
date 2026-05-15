/**
 * Settings → Připojení.
 *
 * Phase 18E: multi-provider/multi-connection management.
 *
 * Shows a card per configured connection (Jira or Freelo) with provider
 * badge, current account label, and basic actions:
 *   - Test connection
 *   - Edit URL / email
 *   - Replace token / API key
 *   - For Freelo: expand the project picker
 *   - Remove (disconnect)
 *
 * The legacy single-Jira section (URL + e-mail + sign out) still appears at
 * the top for backwards compatibility with the original config.toml flow.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Lock, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  getCurrentConfig,
  getFreeloSelectedProjects,
  listConnections,
  listFreeloProjects,
  removeConnection,
  setFreeloSelectedProjects,
  signOut,
  syncFreeloNow,
  updateConfig,
} from "../../api/commands";
import type {
  ConnectionDto,
  FreeloProjectDto,
  JiraConfig,
} from "../../api/types";

export default function Connection() {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState(false);
  const cfgQ = useQuery({
    queryKey: ["current-config"],
    queryFn: getCurrentConfig,
  });
  const connsQ = useQuery({
    queryKey: ["connections"],
    queryFn: listConnections,
  });

  const cfg = cfgQ.data ?? null;
  const initials = (cfg?.email?.[0] ?? "T").toUpperCase();
  const name = displayNameFromEmail(cfg?.email ?? "");

  return (
    <div className="flex flex-col gap-4 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Připojení
        </h2>
      </header>

      <div className="flex flex-col gap-2">
        {/* Legacy Jira account card ----------------------------------------- */}
        {cfg && !editing && (
          <div
            className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                       border border-[var(--border-subtle)] bg-[var(--bg-surface)]"
            data-testid="connection-card-jira-legacy"
          >
            <div
              className="w-8 h-8 rounded-full flex items-center justify-center
                         text-sm font-semibold"
              style={{
                background: "var(--accent-soft)",
                color: "var(--accent)",
              }}
            >
              {initials}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium text-[var(--text-primary)]">
                {name}
              </div>
              {cfg?.base_url && (
                <div className="text-[11px] text-[var(--text-tertiary)] truncate">
                  Jira · {cfg.base_url}
                </div>
              )}
            </div>
            <button
              type="button"
              onClick={() => setEditing(true)}
              aria-label="Upravit připojení"
              className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                         transition-colors duration-150"
            >
              <Pencil className="w-4 h-4" aria-hidden />
            </button>
            <button
              type="button"
              onClick={() => cfgQ.refetch()}
              aria-label="Obnovit připojení"
              className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                         transition-colors duration-150"
            >
              <RefreshCw className="w-4 h-4" aria-hidden />
            </button>
          </div>
        )}
        {cfg && editing && (
          <EditCard
            config={cfg}
            onSaved={() => {
              setEditing(false);
              queryClient.invalidateQueries({ queryKey: ["current-config"] });
            }}
            onCancel={() => setEditing(false)}
          />
        )}

        {/* Multi-connection list (Freelo, additional Jiras, …) -------------- */}
        {(connsQ.data ?? []).map((conn) => (
          <ConnectionCard
            key={conn.id}
            conn={conn}
            onChanged={() =>
              queryClient.invalidateQueries({ queryKey: ["connections"] })
            }
          />
        ))}

        {/* "Add new connection" — opens the setup wizard. */}
        <button
          type="button"
          onClick={() => {
            window.location.hash = "#/setup";
          }}
          className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                     border border-dashed border-[var(--border-subtle)]
                     text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]
                     transition-colors duration-150"
          data-testid="add-connection-button"
        >
          <Plus className="w-4 h-4" aria-hidden />
          <span className="text-sm">Přidat nové připojení</span>
        </button>

        {connsQ.data && connsQ.data.length === 0 && !cfg && (
          <div className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                          border border-[var(--border-subtle)] bg-[var(--bg-surface)]
                          text-[var(--text-tertiary)]"
          >
            <Lock className="w-4 h-4" aria-hidden />
            <span className="text-sm">Žádná připojení nejsou nakonfigurována</span>
          </div>
        )}
      </div>

      {cfg && (
        <button
          type="button"
          onClick={async () => {
            try {
              await signOut();
            } catch {
              /* ignore */
            }
          }}
          className="self-start text-[11px] text-[var(--text-tertiary)] hover:text-[var(--danger)]
                     transition-colors duration-150"
        >
          Odhlásit a odpojit (zastaralé Jira)
        </button>
      )}
    </div>
  );
}

function providerLabel(p: string): string {
  if (p === "jira") return "Jira";
  if (p === "freelo") return "Freelo";
  return p;
}

function ConnectionCard({
  conn,
  onChanged,
}: {
  conn: ConnectionDto;
  onChanged: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const email =
    typeof conn.config["email"] === "string"
      ? (conn.config["email"] as string)
      : null;
  const initials = (email?.[0] ?? conn.provider[0] ?? "?").toUpperCase();

  return (
    <div
      className="flex flex-col gap-2 p-3 rounded-[var(--radius-md)]
                 border border-[var(--border-subtle)] bg-[var(--bg-surface)]"
      data-testid={`connection-card-${conn.id}`}
    >
      <div className="flex items-center gap-3">
        <div
          className="w-8 h-8 rounded-full flex items-center justify-center
                     text-sm font-semibold"
          style={{
            background: "var(--accent-soft)",
            color: "var(--accent)",
          }}
        >
          {initials}
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-[var(--text-primary)]">
            {conn.name}
          </div>
          <div className="text-[11px] text-[var(--text-tertiary)] truncate">
            {providerLabel(conn.provider)}
            {email ? ` · ${email}` : ""}
          </div>
        </div>
        <button
          type="button"
          aria-label="Odpojit"
          onClick={async () => {
            if (!window.confirm(`Odpojit ${conn.name}?`)) return;
            try {
              await removeConnection(conn.id);
              onChanged();
            } catch {
              /* ignore */
            }
          }}
          className="text-[var(--text-tertiary)] hover:text-[var(--danger)]
                     transition-colors duration-150"
        >
          <Trash2 className="w-4 h-4" aria-hidden />
        </button>
      </div>

      {conn.provider === "freelo" && (
        <>
          <button
            type="button"
            onClick={() => setExpanded((e) => !e)}
            className="self-start text-xs text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors duration-150"
          >
            {expanded ? "Skrýt projekty" : "Vybrané projekty"}
          </button>
          {expanded && <FreeloProjectsPanel connectionId={conn.id} />}
        </>
      )}
    </div>
  );
}

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
        setProjects(
          list.map((p) => ({ ...p, selected: sel.has(p.id) })),
        );
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
      <p className="text-xs text-[var(--text-tertiary)]">Načítám projekty…</p>
    );
  }
  if (error) {
    return (
      <p className="text-xs text-[var(--danger)]" role="alert">
        {error}
      </p>
    );
  }
  if (projects.length === 0) {
    return (
      <p className="text-xs text-[var(--text-tertiary)]">
        Žádné projekty nenalezeny.
      </p>
    );
  }
  return (
    <ul
      className="flex flex-col gap-0.5 max-h-[240px] overflow-y-auto"
      data-testid={`freelo-projects-${connectionId}`}
    >
      {projects.map((p) => (
        <li key={p.id}>
          <label className="flex items-center gap-2 px-1 py-1 text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] rounded-[var(--radius-sm)] cursor-pointer">
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
  );
}

function EditCard({
  config,
  onSaved,
  onCancel,
}: {
  config: JiraConfig | null;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const [baseUrl, setBaseUrl] = useState(config?.base_url ?? "");
  const [email, setEmail] = useState(config?.email ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setError(null);
    setSaving(true);
    try {
      await updateConfig({ base_url: baseUrl, email }, null);
      onSaved();
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to save");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="p-3 rounded-[var(--radius-md)] border border-[var(--border-subtle)]
                    bg-[var(--bg-surface)] flex flex-col gap-2">
      <input
        type="url"
        value={baseUrl}
        onChange={(e) => setBaseUrl(e.target.value)}
        className={inputCls}
        placeholder="https://your-org.atlassian.net"
      />
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        className={inputCls}
        placeholder="you@example.com"
      />
      {error && <div className="text-xs text-[var(--danger)]">{error}</div>}
      <div className="flex gap-2 mt-1">
        <button
          type="button"
          onClick={save}
          disabled={saving}
          className="inline-flex items-center px-3 h-8 rounded-[var(--radius-md)]
                     bg-[var(--accent)] text-[var(--accent-text)] text-xs
                     hover:bg-[var(--accent-hover)] transition-colors duration-150
                     disabled:opacity-60"
        >
          Uložit změny
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="inline-flex items-center px-3 h-8 rounded-[var(--radius-md)]
                     border border-[var(--border-subtle)] text-xs text-[var(--text-secondary)]
                     hover:bg-[var(--bg-hover)] transition-colors duration-150"
        >
          Zrušit
        </button>
      </div>
    </div>
  );
}

const inputCls =
  "w-full h-8 px-2.5 rounded-[var(--radius-sm)] bg-transparent " +
  "border border-[var(--border-subtle)] text-sm text-[var(--text-primary)] " +
  "focus:outline-none focus:border-[var(--border-default)] " +
  "transition-colors duration-150";

function displayNameFromEmail(email: string): string {
  if (!email) return "Nepřipojeno";
  const [local] = email.split("@");
  if (!local) return email;
  return local
    .split(/[._-]/)
    .filter(Boolean)
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join(" ");
}

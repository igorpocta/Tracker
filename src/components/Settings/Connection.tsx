/**
 * Settings → Connection Setup.
 *
 * Reference: `screens/SCR-20260514-rjmh-2.png`.
 *
 *   ┌──────────────────────────────────────────────┐
 *   │  ⓘ  Igor Počta                          ✏  ↻ │
 *   └──────────────────────────────────────────────┘
 *   ┌──────────────────────────────────────────────┐
 *   │  🔒  Add new connection                      │   (disabled)
 *   └──────────────────────────────────────────────┘
 *
 * The account card shows the configured Jira identity. Edit (pencil) opens
 * the existing inline edit form (URL + email); the refresh icon re-pulls
 * `myself` to update the display name. "Add new connection" is locked —
 * multi-tenant support is a premium feature we don't pretend to offer here.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Lock, Pencil, RefreshCw } from "lucide-react";
import { useState } from "react";

import { getCurrentConfig, signOut, updateConfig } from "../../api/commands";
import type { JiraConfig } from "../../api/types";

export default function Connection() {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState(false);
  const cfgQ = useQuery({
    queryKey: ["current-config"],
    queryFn: getCurrentConfig,
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
        {/* Account card */}
        {!editing ? (
          <div
            className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                       border border-[var(--border-subtle)] bg-[var(--bg-surface)]"
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
                  {cfg.base_url}
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
        ) : (
          <EditCard
            config={cfg}
            onSaved={() => {
              setEditing(false);
              queryClient.invalidateQueries({ queryKey: ["current-config"] });
            }}
            onCancel={() => setEditing(false)}
          />
        )}

        {/* "Add new connection" — locked */}
        <div
          aria-disabled="true"
          className="flex items-center gap-3 p-3 rounded-[var(--radius-md)]
                     border border-dashed border-[var(--border-subtle)]
                     text-[var(--text-tertiary)] opacity-70 cursor-not-allowed"
        >
          <Lock className="w-4 h-4" aria-hidden />
          <span className="text-sm">Přidat nové připojení</span>
        </div>
      </div>

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
        Odhlásit a odpojit
      </button>
    </div>
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

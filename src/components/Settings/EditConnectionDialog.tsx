/**
 * Inline "Edit connection" modal used from Settings → Připojení.
 *
 * Phase 18F: the multi-provider settings UI exposes a Pencil icon on every
 * connection card. Clicking it opens this dialog pre-populated with the
 * connection's current name, base URL / email, and a "Replace token / API key"
 * button that reveals an input only when the user explicitly wants to rotate
 * the secret.
 *
 * Fields:
 *   - Jira:   name, base URL, email, optional new token.
 *   - Freelo: name, email, advanced base URL, optional new API key.
 *
 * The Test button (re)verifies the supplied credentials against the provider.
 * The Save button calls `update_connection` (and `update_config` legacy for
 * the first Jira, just to keep the keychain happy).
 */
import { CircleCheck, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";

import {
  listJiraStatuses,
  testConnectionForProvider,
  testJiraConnection,
  updateConnectionApi,
} from "../../api/commands";
import type { ConnectionDto, ProviderUser } from "../../api/types";
import {
  emailSchema,
  freeloApiKeySchema,
  tokenSchema,
  urlSchema,
} from "../../lib/validation";

export interface EditConnectionDialogProps {
  open: boolean;
  conn: ConnectionDto;
  onClose: () => void;
  onSaved: () => void;
}

type TestState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ok"; user: { displayName: string } }
  | { kind: "error"; message: string };

export function EditConnectionDialog({
  open,
  conn,
  onClose,
  onSaved,
}: EditConnectionDialogProps) {
  const cfg = (conn.config ?? {}) as Record<string, unknown>;
  const initialUrl = typeof cfg["base_url"] === "string" ? (cfg["base_url"] as string) : "";
  const initialEmail = typeof cfg["email"] === "string" ? (cfg["email"] as string) : "";
  const initialDashboardEnabled = cfg["dashboard_enabled"] === true;
  const initialDashboardJql =
    typeof cfg["dashboard_jql"] === "string" ? (cfg["dashboard_jql"] as string) : "";
  const initialAutoFrom =
    typeof cfg["auto_transition_from"] === "string"
      ? (cfg["auto_transition_from"] as string)
      : "";
  const initialAutoTo =
    typeof cfg["auto_transition_to_name"] === "string"
      ? (cfg["auto_transition_to_name"] as string)
      : "";
  const initialColor =
    typeof cfg["color"] === "string" ? (cfg["color"] as string) : "";

  const [name, setName] = useState(conn.name);
  const [baseUrl, setBaseUrl] = useState(initialUrl);
  const [email, setEmail] = useState(initialEmail);
  const [showSecret, setShowSecret] = useState(false);
  const [secret, setSecret] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dashboardEnabled, setDashboardEnabled] = useState(initialDashboardEnabled);
  const [dashboardJql, setDashboardJql] = useState(initialDashboardJql);
  const [autoFrom, setAutoFrom] = useState(initialAutoFrom);
  const [autoTo, setAutoTo] = useState(initialAutoTo);
  // Seznam Jira status názvů (lazy-loaded přes /rest/api/3/status).
  const [statuses, setStatuses] = useState<string[] | null>(null);
  const [statusesLoading, setStatusesLoading] = useState(false);
  const [statusesError, setStatusesError] = useState<string | null>(null);
  /** Hex barva pro toto připojení; prázdný řetězec = "fallback default". */
  const [color, setColor] = useState(initialColor);
  const [colorEnabled, setColorEnabled] = useState(initialColor.length > 0);

  const isJira = conn.provider === "jira";
  const isFreelo = conn.provider === "freelo";

  // Statusy nahrajeme až když uživatel detaily otevře. `loadStatuses` je
  // idempotentní — po prvním fetchnutí se další volání no-op-uje.
  async function loadStatuses() {
    if (!isJira || statuses !== null || statusesLoading) return;
    setStatusesLoading(true);
    setStatusesError(null);
    try {
      const list = await listJiraStatuses(conn.id);
      setStatuses(list);
    } catch (e) {
      setStatusesError(errMsg(e, "Statusy se nepodařilo načíst"));
      setStatuses([]); // neretryovat při každém open
    } finally {
      setStatusesLoading(false);
    }
  }
  // Pokud existuje uložená hodnota, statusy hned hydratujeme — uživatel
  // nemusí klikat na <details> aby viděl předvyplněný select.
  useEffect(() => {
    if (open && isJira && (initialAutoFrom || initialAutoTo)) {
      void loadStatuses();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  // Validation: name required; URL / email required only if changed.
  const nameOk = name.trim().length > 0;
  const urlOk = !isJira || urlSchema.safeParse(baseUrl).success;
  const emailOk = emailSchema.safeParse(email).success;
  const secretOk =
    !showSecret ||
    (isJira
      ? tokenSchema.safeParse(secret).success
      : freeloApiKeySchema.safeParse(secret).success);

  const canSave = nameOk && urlOk && emailOk && secretOk && !saving;
  const canTest =
    nameOk &&
    urlOk &&
    emailOk &&
    showSecret &&
    secretOk &&
    test.kind !== "loading";

  async function handleTest() {
    setError(null);
    setTest({ kind: "loading" });
    try {
      if (isJira) {
        const user = await testJiraConnection(baseUrl, email, secret);
        setTest({ kind: "ok", user });
      } else {
        const user: ProviderUser = await testConnectionForProvider({
          provider: "freelo",
          config: { base_url: baseUrl || "https://api.freelo.io/v1", email },
          token: secret,
        });
        setTest({ kind: "ok", user });
      }
    } catch (e) {
      setTest({
        kind: "error",
        message: errMsg(e, "Připojení se nezdařilo"),
      });
    }
  }

  async function handleSave() {
    setError(null);
    setSaving(true);
    try {
      const newConfig: Record<string, unknown> = { ...cfg };
      if (isJira) {
        newConfig["base_url"] = baseUrl;
        newConfig["email"] = email;
        newConfig["dashboard_enabled"] = dashboardEnabled;
        newConfig["dashboard_jql"] = dashboardJql.trim() || null;
        newConfig["auto_transition_from"] = autoFrom.trim() || null;
        newConfig["auto_transition_to_name"] = autoTo.trim() || null;
      } else if (isFreelo) {
        newConfig["email"] = email;
        if (baseUrl) newConfig["base_url"] = baseUrl;
      }
      // Color je provider-agnostický override; null = používej default.
      newConfig["color"] = colorEnabled && color ? color : null;
      await updateConnectionApi({
        id: conn.id,
        name: name.trim(),
        config: newConfig,
        token: showSecret && secret.length > 0 ? secret : undefined,
      });
      onSaved();
      handleClose();
    } catch (e) {
      setError(errMsg(e, "Uložení se nezdařilo"));
    } finally {
      setSaving(false);
    }
  }

  function handleClose() {
    setShowSecret(false);
    setSecret("");
    setError(null);
    setTest({ kind: "idle" });
    onClose();
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Upravit ${conn.name}`}
      data-testid="edit-connection-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) handleClose();
      }}
    >
      <div
        className="w-[460px] max-w-[92vw] max-h-[88vh] overflow-y-auto p-5
                   rounded-[var(--radius-lg)] flex flex-col gap-4"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
        }}
      >
        <header className="flex items-center justify-between">
          <h3 className="text-base font-semibold text-[var(--text-primary)]">
            Upravit {providerLabel(conn.provider)} připojení
          </h3>
          <button
            type="button"
            onClick={handleClose}
            aria-label="Zavřít"
            className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                       transition-colors duration-150 text-xl leading-none px-1"
          >
            ×
          </button>
        </header>

        <Field
          id="edit-conn-name"
          label="Název připojení"
          type="text"
          value={name}
          onChange={setName}
          placeholder="např. SAB, Klient X"
        />

        {isJira && (
          <Field
            id="edit-jira-url"
            label="Základní URL Jiry"
            type="url"
            value={baseUrl}
            onChange={(v) => {
              setBaseUrl(v);
              if (test.kind !== "idle") setTest({ kind: "idle" });
            }}
          />
        )}

        <Field
          id="edit-conn-email"
          label={isJira ? "E-mail Atlassian účtu" : "Freelo e-mail"}
          type="email"
          value={email}
          autoComplete="email"
          onChange={(v) => {
            setEmail(v);
            if (test.kind !== "idle") setTest({ kind: "idle" });
          }}
        />

        {isFreelo && (
          <>
            <button
              type="button"
              onClick={() => setAdvanced((a) => !a)}
              className="self-start text-xs text-[var(--text-tertiary)]
                         hover:text-[var(--text-primary)] transition-colors duration-150"
            >
              {advanced
                ? "Skrýt pokročilá nastavení"
                : "Zobrazit pokročilá nastavení"}
            </button>
            {advanced && (
              <Field
                id="edit-freelo-base-url"
                label="Freelo API URL"
                type="url"
                value={baseUrl}
                placeholder="https://api.freelo.io/v1"
                onChange={(v) => {
                  setBaseUrl(v);
                  if (test.kind !== "idle") setTest({ kind: "idle" });
                }}
              />
            )}
          </>
        )}

        {!showSecret && (
          <button
            type="button"
            onClick={() => setShowSecret(true)}
            data-testid="edit-conn-replace-secret"
            className="self-start text-xs text-[var(--text-tertiary)]
                       hover:text-[var(--text-primary)] transition-colors duration-150"
          >
            {isJira ? "Nahradit API token" : "Nahradit API klíč"}
          </button>
        )}

        {showSecret && (
          <Field
            id="edit-conn-secret"
            label={isJira ? "Nový Jira API token" : "Nový Freelo API klíč"}
            type="password"
            mono
            value={secret}
            onChange={(v) => {
              setSecret(v);
              if (test.kind !== "idle") setTest({ kind: "idle" });
            }}
          />
        )}

        {showSecret && (
          <div className="flex items-center gap-3 flex-wrap">
            <button
              type="button"
              onClick={() => void handleTest()}
              disabled={!canTest}
              className="h-9 px-4 rounded-[var(--radius-md)] border border-[var(--border-default)]
                         hover:bg-[var(--bg-hover)] disabled:opacity-40 disabled:cursor-not-allowed
                         text-sm font-medium text-[var(--text-primary)]
                         transition-colors duration-150 flex items-center gap-2"
            >
              {test.kind === "loading" && (
                <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
              )}
              Otestovat
            </button>
            {test.kind === "ok" && (
              <span
                className="flex items-center gap-1.5 text-xs text-[var(--success)]"
                role="status"
              >
                <CircleCheck className="w-4 h-4" aria-hidden />
                Připojeno jako {test.user.displayName}
              </span>
            )}
            {test.kind === "error" && (
              <span className="text-xs text-[var(--danger)]" role="alert">
                {test.message}
              </span>
            )}
          </div>
        )}

        {isJira && (
          <div className="flex flex-col gap-2 pt-2 border-t border-[var(--border-subtle)]">
            <label className="flex items-start gap-2 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={dashboardEnabled}
                onChange={(e) => setDashboardEnabled(e.target.checked)}
                className="mt-0.5 accent-[var(--accent)]"
              />
              <span className="text-xs text-[var(--text-secondary)]">
                <span className="font-medium text-[var(--text-primary)]">
                  Zobrazit Dashboard
                </span>
                <br />
                Přidá tuto Jiru do přehledové tabulky „JIRA Přehled" v menu.
                Vyžaduje JQL filter níže.
              </span>
            </label>
            {dashboardEnabled && (
              <div className="flex flex-col gap-1">
                <label
                  htmlFor="edit-jira-dashboard-jql"
                  className="text-xs font-medium text-[var(--text-secondary)]"
                >
                  JQL pro Dashboard
                </label>
                <textarea
                  id="edit-jira-dashboard-jql"
                  value={dashboardJql}
                  onChange={(e) => setDashboardJql(e.target.value)}
                  placeholder={
                    'project = "PORTAL" AND statusCategory != "Done" ORDER BY priority DESC'
                  }
                  spellCheck={false}
                  rows={3}
                  className="px-3 py-2 rounded-[var(--radius-md)] bg-transparent
                             border border-[var(--border-default)]
                             focus:border-[var(--accent)] focus:outline-none
                             focus:ring-2 focus:ring-[var(--accent-ring)]
                             text-xs font-mono text-[var(--text-primary)]
                             transition-colors duration-150 resize-y"
                />
                <p className="text-[10px] text-[var(--text-tertiary)]">
                  Atlassian odmítne JQL bez aspoň jedné restrikce. Bez ORDER BY
                  bere defaultní řazení dle Jiry.
                </p>
              </div>
            )}

            <details
              className="mt-2"
              onToggle={(e) => {
                if ((e.target as HTMLDetailsElement).open) void loadStatuses();
              }}
            >
              <summary className="text-xs text-[var(--text-secondary)] cursor-pointer hover:text-[var(--text-primary)]">
                Automatický přechod stavu (volitelné)
              </summary>
              <div className="grid grid-cols-2 gap-3 mt-2">
                <StatusSelect
                  id="auto-trans-from"
                  label="Pokud je úkol ve stavu…"
                  value={autoFrom}
                  options={statuses}
                  loading={statusesLoading}
                  onChange={setAutoFrom}
                />
                <StatusSelect
                  id="auto-trans-to"
                  label="…přejít při spuštění na"
                  value={autoTo}
                  options={statuses}
                  loading={statusesLoading}
                  onChange={setAutoTo}
                />
              </div>
              {statusesError && (
                <p className="text-[10px] text-[var(--danger,#dc2626)] mt-1">
                  {statusesError}
                </p>
              )}
              <p className="text-[10px] text-[var(--text-tertiary)] mt-1">
                Tichá best-effort akce — pokud mezi vybranými stavy v Jiře
                neexistuje přímá transition, Tracker se ji prostě nepokusí
                provést (zapíše do logu). Necháte-li vybráno „—", nic se nedělá.
              </p>
            </details>
          </div>
        )}

        <section className="flex flex-col gap-2 pt-2 border-t border-[var(--border-subtle)]">
          <label className="flex items-start gap-2 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={colorEnabled}
              onChange={(e) => {
                setColorEnabled(e.target.checked);
                if (e.target.checked && !color) {
                  setColor(isJira ? "#1B6FE5" : "#2CC067");
                }
              }}
              className="mt-0.5 accent-[var(--accent)]"
            />
            <span className="text-xs text-[var(--text-secondary)]">
              <span className="font-medium text-[var(--text-primary)]">
                Vlastní barva v Reportech
              </span>
              <br />
              Když je vypnuto, použije se výchozí barva providera.
            </span>
          </label>
          {colorEnabled && (
            <div className="flex items-center gap-2 pl-6">
              <input
                type="color"
                value={color || "#1B6FE5"}
                onChange={(e) => setColor(e.target.value)}
                className="w-8 h-8 rounded border-none cursor-pointer bg-transparent"
                aria-label="Barva pro toto připojení"
              />
              <span className="font-mono text-[11px] text-[var(--text-tertiary)]">
                {color || "#1B6FE5"}
              </span>
            </div>
          )}
        </section>

        {error && (
          <p className="text-xs text-[var(--danger)]" role="alert">
            {error}
          </p>
        )}

        <div className="flex items-center justify-end gap-2 mt-1">
          <button
            type="button"
            onClick={handleClose}
            disabled={saving}
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       text-[var(--text-secondary)]
                       hover:bg-[var(--bg-hover)]
                       transition-colors duration-150"
          >
            Zrušit
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={!canSave}
            data-testid="edit-conn-save"
            className="h-8 px-3 rounded-[var(--radius-md)] text-sm
                       font-semibold transition-colors duration-150
                       disabled:opacity-50 flex items-center gap-2"
            style={{
              background: "var(--accent)",
              color: "var(--accent-text, #fff)",
            }}
          >
            {saving && (
              <LoaderCircle className="w-3.5 h-3.5 animate-spin" aria-hidden />
            )}
            Uložit
          </button>
        </div>
      </div>
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

function errMsg(e: unknown, fallback: string): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return fallback;
}

function StatusSelect({
  id,
  label,
  value,
  options,
  loading,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  options: string[] | null;
  loading: boolean;
  onChange: (v: string) => void;
}) {
  // Pokud má uživatel uloženou hodnotu, která už ve statuses není (jiný
  // workflow, přejmenovaný status), zachováme ji jako fallback option, ať
  // se select nepřeskočí na prázdno.
  const merged: string[] = (() => {
    const base = options ?? [];
    if (value && !base.includes(value)) return [value, ...base];
    return base;
  })();
  return (
    <div className="flex flex-col gap-1">
      <label
        htmlFor={id}
        className="text-xs font-medium text-[var(--text-secondary)]"
      >
        {label}
      </label>
      <select
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={loading}
        className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent
                   border border-[var(--border-default)] focus:border-[var(--accent)]
                   focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                   text-sm text-[var(--text-primary)] transition-colors duration-150
                   disabled:opacity-60"
      >
        <option value="">— nezvoleno —</option>
        {loading && options === null && <option disabled>Načítám…</option>}
        {merged.map((s) => (
          <option key={s} value={s}>
            {s}
          </option>
        ))}
      </select>
    </div>
  );
}

function Field({
  id,
  label,
  type,
  placeholder,
  value,
  onChange,
  autoComplete,
  mono,
}: {
  id: string;
  label: string;
  type: "text" | "url" | "email" | "password";
  placeholder?: string;
  value: string;
  onChange: (v: string) => void;
  autoComplete?: string;
  mono?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label
        htmlFor={id}
        className="text-xs font-medium text-[var(--text-secondary)]"
      >
        {label}
      </label>
      <input
        id={id}
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoComplete={autoComplete ?? "off"}
        spellCheck={false}
        className={
          "px-3 h-9 rounded-[var(--radius-md)] bg-transparent " +
          "border border-[var(--border-default)] focus:border-[var(--accent)] " +
          "focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] " +
          "text-sm text-[var(--text-primary)] transition-colors duration-150 " +
          (mono ? "font-mono" : "")
        }
      />
    </div>
  );
}

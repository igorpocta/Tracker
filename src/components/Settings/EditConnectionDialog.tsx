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
import { useState } from "react";

import {
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

  const [name, setName] = useState(conn.name);
  const [baseUrl, setBaseUrl] = useState(initialUrl);
  const [email, setEmail] = useState(initialEmail);
  const [showSecret, setShowSecret] = useState(false);
  const [secret, setSecret] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const isJira = conn.provider === "jira";
  const isFreelo = conn.provider === "freelo";

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
      } else if (isFreelo) {
        newConfig["email"] = email;
        if (baseUrl) newConfig["base_url"] = baseUrl;
      }
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

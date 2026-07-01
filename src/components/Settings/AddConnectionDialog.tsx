/**
 * Inline "Add new connection" modal used from Settings → Připojení.
 *
 * Phase 18F replaces the legacy "navigate the user to the fullscreen Setup
 * wizard" flow with an in-Settings dialog so:
 *   1. The user is never redirected away from the settings panel.
 *   2. The flow shows the same multi-provider picker as the fresh-install
 *      wizard but adds a connection-name field at the top.
 *   3. The dialog reuses the `Step*` components from `src/components/Setup/`
 *      so the credentials / project picker UI is identical to the wizard.
 *
 * The dialog has three logical steps:
 *
 *   step 0  Pick provider (Jira / Freelo). Toggl + Clockify are visible but
 *           disabled ("Brzy" badge).
 *   step 1  Provider-specific credentials form (URL/email/token for Jira,
 *           email/api-key for Freelo). User can Test + Save.
 *   step 2  Freelo only — project picker.
 *
 * After Done the dialog closes and the parent invalidates the
 * `queryKeys.connections.all()` query so the new card shows up immediately.
 */
import { CircleCheck, LoaderCircle } from "lucide-react";
import { useState } from "react";

import {
  addConnection,
  listFreeloProjects,
  setFreeloSelectedProjects,
  syncFreeloNow,
  testConnectionForProvider,
} from "../../api/commands";
import type { ProviderKind, ProviderUser } from "../../api/types";
import {
  emailSchema,
  freeloApiKeySchema,
  tokenSchema,
  urlSchema,
} from "../../lib/validation";
import { StepFreeloProjects } from "../Setup/StepFreeloProjects";
import { StepProvider } from "../Setup/StepProvider";

export interface AddConnectionDialogProps {
  open: boolean;
  onClose: () => void;
  /** Called after a connection (and, for Freelo, projects) is successfully saved. */
  onSaved: () => void;
}

type Step = "provider" | "creds" | "freelo-projects";

export function AddConnectionDialog({
  open,
  onClose,
  onSaved,
}: AddConnectionDialogProps) {
  const [step, setStep] = useState<Step>("provider");
  const [provider, setProvider] = useState<ProviderKind | null>(null);

  // Form fields — single set, branching by `provider`.
  const [name, setName] = useState("");
  const [jiraUrl, setJiraUrl] = useState("");
  const [jiraEmail, setJiraEmail] = useState("");
  const [jiraToken, setJiraToken] = useState("");
  const [jiraAllowCustomHost, setJiraAllowCustomHost] = useState(false);
  const [freeloEmail, setFreeloEmail] = useState("");
  const [freeloApiKey, setFreeloApiKey] = useState("");
  const [freeloBaseUrl, setFreeloBaseUrl] = useState(
    "https://api.freelo.io/v1",
  );
  const [freeloBaseUrlAdvanced, setFreeloBaseUrlAdvanced] = useState(false);

  const [test, setTest] = useState<
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ok"; user: ProviderUser | { displayName: string } }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [freeloConnectionId, setFreeloConnectionId] = useState<number | null>(
    null,
  );

  if (!open) return null;

  function reset() {
    setStep("provider");
    setProvider(null);
    setName("");
    setJiraUrl("");
    setJiraEmail("");
    setJiraToken("");
    setJiraAllowCustomHost(false);
    setFreeloEmail("");
    setFreeloApiKey("");
    setFreeloBaseUrl("https://api.freelo.io/v1");
    setFreeloBaseUrlAdvanced(false);
    setTest({ kind: "idle" });
    setError(null);
    setSaving(false);
    setFreeloConnectionId(null);
  }

  function close() {
    reset();
    onClose();
  }

  function chooseProvider(p: ProviderKind) {
    setProvider(p);
    if (p === "jira") setName("Jira");
    if (p === "freelo") setName("Freelo");
  }

  // ---------- Jira credentials submit (test → save) -----------------------
  async function jiraTest() {
    setError(null);
    setTest({ kind: "loading" });
    try {
      // Route through the provider-generic probe so the backend honours
      // `allow_custom_host`; the legacy `test_jira_connection` path hard-codes
      // the cloud-only allow-list and would reject a self-hosted URL here even
      // though Save (which does pass the flag) would have accepted it.
      const user = await testConnectionForProvider({
        provider: "jira",
        config: {
          base_url: jiraUrl,
          email: jiraEmail,
          allow_custom_host: jiraAllowCustomHost,
        },
        token: jiraToken,
      });
      setTest({ kind: "ok", user });
    } catch (e) {
      setTest({
        kind: "error",
        message: errMsg(e, "Připojení se nezdařilo"),
      });
    }
  }

  async function jiraSave() {
    setError(null);
    setSaving(true);
    try {
      await addConnection({
        provider: "jira",
        name: name.trim() || "Jira",
        config: {
          base_url: jiraUrl,
          email: jiraEmail,
          allow_custom_host: jiraAllowCustomHost,
        },
        token: jiraToken,
      });
      onSaved();
      close();
    } catch (e) {
      setError(errMsg(e, "Uložení se nezdařilo"));
    } finally {
      setSaving(false);
    }
  }

  // ---------- Freelo credentials submit (test → save + advance) -----------
  async function freeloTest() {
    setError(null);
    setTest({ kind: "loading" });
    try {
      const user = await testConnectionForProvider({
        provider: "freelo",
        config: { base_url: freeloBaseUrl, email: freeloEmail },
        token: freeloApiKey,
      });
      setTest({ kind: "ok", user });
    } catch (e) {
      setTest({
        kind: "error",
        message: errMsg(e, "Připojení se nezdařilo"),
      });
    }
  }

  async function freeloSaveAndAdvance() {
    setError(null);
    setSaving(true);
    try {
      const user =
        test.kind === "ok" ? (test.user as ProviderUser) : null;
      const conn = await addConnection({
        provider: "freelo",
        name: name.trim() || "Freelo",
        config: {
          base_url: freeloBaseUrl,
          email: freeloEmail,
          selected_project_ids: [],
          sync_user_id: user ? Number(user.accountId) : undefined,
        },
        token: freeloApiKey,
      });
      setFreeloConnectionId(conn.id);
      setStep("freelo-projects");
    } catch (e) {
      setError(errMsg(e, "Uložení se nezdařilo"));
    } finally {
      setSaving(false);
    }
  }

  async function freeloFinishProjects(projectIds: number[]) {
    if (freeloConnectionId == null) return;
    setError(null);
    try {
      await setFreeloSelectedProjects(freeloConnectionId, projectIds);
      // Best-effort: kick off an initial sync.
      try {
        await syncFreeloNow(freeloConnectionId);
      } catch {
        /* ignore */
      }
      onSaved();
      close();
    } catch (e) {
      setError(errMsg(e, "Uložení se nezdařilo"));
    }
  }

  // -------------------- shape validation flags ----------------------------
  const nameOk = name.trim().length > 0;
  const jiraShapeOk =
    urlSchema.safeParse(jiraUrl).success &&
    emailSchema.safeParse(jiraEmail).success &&
    tokenSchema.safeParse(jiraToken).success;
  const freeloShapeOk =
    emailSchema.safeParse(freeloEmail).success &&
    freeloApiKeySchema.safeParse(freeloApiKey).success;

  const canTestJira = jiraShapeOk && test.kind !== "loading";
  const canSaveJira = test.kind === "ok" && nameOk && !saving;
  const canTestFreelo = freeloShapeOk && test.kind !== "loading";
  const canSaveFreelo = test.kind === "ok" && nameOk && !saving;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Přidat nové připojení"
      data-testid="add-connection-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.4)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
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
            {step === "provider" && "Vyberte poskytovatele"}
            {step === "creds" && providerLabel(provider) + " — přihlášení"}
            {step === "freelo-projects" && "Vyberte projekty"}
          </h3>
          <button
            type="button"
            onClick={close}
            aria-label="Zavřít"
            className="text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                       transition-colors duration-150 text-xl leading-none px-1"
          >
            ×
          </button>
        </header>

        {step === "provider" && (
          <StepProvider
            value={provider}
            onChange={chooseProvider}
            onNext={() => setStep("creds")}
          />
        )}

        {step === "creds" && provider === "jira" && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (canSaveJira) void jiraSave();
              else if (canTestJira) void jiraTest();
            }}
            className="flex flex-col gap-3"
          >
            <NameField value={name} onChange={setName} />

            <Field
              id="add-jira-url"
              label="Základní URL Jiry"
              type="url"
              placeholder="https://acme.atlassian.net"
              value={jiraUrl}
              onChange={(v) => {
                setJiraUrl(v);
                if (test.kind !== "idle") setTest({ kind: "idle" });
              }}
            />
            <Field
              id="add-jira-email"
              label="E-mail Atlassian účtu"
              type="email"
              placeholder="you@example.com"
              autoComplete="email"
              value={jiraEmail}
              onChange={(v) => {
                setJiraEmail(v);
                if (test.kind !== "idle") setTest({ kind: "idle" });
              }}
            />
            <Field
              id="add-jira-token"
              label="Jira API token"
              type="password"
              placeholder="vložte svůj token"
              mono
              value={jiraToken}
              onChange={(v) => {
                setJiraToken(v);
                if (test.kind !== "idle") setTest({ kind: "idle" });
              }}
            />

            <label className="flex items-start gap-2 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={jiraAllowCustomHost}
                onChange={(e) => {
                  setJiraAllowCustomHost(e.target.checked);
                  if (test.kind !== "idle") setTest({ kind: "idle" });
                }}
                className="mt-0.5 accent-[var(--accent)]"
              />
              <span className="text-xs text-[var(--text-secondary)]">
                <span className="font-medium text-[var(--text-primary)]">
                  Vlastní / self-hosted server
                </span>
                <br />
                Zapněte pro on-premise Jiru mimo <code>*.atlassian.net</code>.
                Ověřte, že URL je důvěryhodná — token se odešle na tento server.
              </span>
            </label>

            <div className="flex items-center gap-3 flex-wrap">
              <button
                type="button"
                onClick={() => void jiraTest()}
                disabled={!canTestJira}
                data-testid="add-conn-test"
                className="h-9 px-4 rounded-[var(--radius-md)] border
                           border-[var(--border-default)] hover:bg-[var(--bg-hover)]
                           disabled:opacity-40 disabled:cursor-not-allowed
                           text-sm font-medium text-[var(--text-primary)]
                           transition-colors duration-150 flex items-center gap-2"
              >
                {test.kind === "loading" && (
                  <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
                )}
                Otestovat
              </button>
              <TestStatus test={test} />
            </div>

            {error && (
              <p className="text-xs text-[var(--danger)]" role="alert">
                {error}
              </p>
            )}

            <FooterButtons
              onBack={() => {
                setStep("provider");
                setTest({ kind: "idle" });
                setError(null);
              }}
              onPrimary={() => void jiraSave()}
              primaryLabel="Uložit"
              primaryDisabled={!canSaveJira}
              primaryLoading={saving}
              primaryTestId="add-conn-save-jira"
            />
          </form>
        )}

        {step === "creds" && provider === "freelo" && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (canSaveFreelo) void freeloSaveAndAdvance();
              else if (canTestFreelo) void freeloTest();
            }}
            className="flex flex-col gap-3"
          >
            <NameField value={name} onChange={setName} />

            <Field
              id="add-freelo-email"
              label="Freelo e-mail"
              type="email"
              placeholder="you@example.com"
              autoComplete="email"
              value={freeloEmail}
              onChange={(v) => {
                setFreeloEmail(v);
                if (test.kind !== "idle") setTest({ kind: "idle" });
              }}
            />
            <Field
              id="add-freelo-key"
              label="Freelo API klíč"
              type="password"
              placeholder="vložte svůj API klíč"
              mono
              value={freeloApiKey}
              onChange={(v) => {
                setFreeloApiKey(v);
                if (test.kind !== "idle") setTest({ kind: "idle" });
              }}
            />

            <button
              type="button"
              onClick={() => setFreeloBaseUrlAdvanced((a) => !a)}
              className="self-start text-xs text-[var(--text-tertiary)]
                         hover:text-[var(--text-primary)] transition-colors duration-150"
            >
              {freeloBaseUrlAdvanced
                ? "Skrýt pokročilá nastavení"
                : "Zobrazit pokročilá nastavení"}
            </button>

            {freeloBaseUrlAdvanced && (
              <Field
                id="add-freelo-base-url"
                label="Freelo API URL"
                type="url"
                placeholder="https://api.freelo.io/v1"
                value={freeloBaseUrl}
                onChange={setFreeloBaseUrl}
              />
            )}

            <div className="flex items-center gap-3 flex-wrap">
              <button
                type="button"
                onClick={() => void freeloTest()}
                disabled={!canTestFreelo}
                className="h-9 px-4 rounded-[var(--radius-md)] border
                           border-[var(--border-default)] hover:bg-[var(--bg-hover)]
                           disabled:opacity-40 disabled:cursor-not-allowed
                           text-sm font-medium text-[var(--text-primary)]
                           transition-colors duration-150 flex items-center gap-2"
              >
                {test.kind === "loading" && (
                  <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
                )}
                Otestovat
              </button>
              <TestStatus test={test} />
            </div>

            {error && (
              <p className="text-xs text-[var(--danger)]" role="alert">
                {error}
              </p>
            )}

            <FooterButtons
              onBack={() => {
                setStep("provider");
                setTest({ kind: "idle" });
                setError(null);
              }}
              onPrimary={() => void freeloSaveAndAdvance()}
              primaryLabel="Pokračovat"
              primaryDisabled={!canSaveFreelo}
              primaryLoading={saving}
              primaryTestId="add-conn-save-freelo"
            />
          </form>
        )}

        {step === "freelo-projects" && freeloConnectionId != null && (
          <>
            <StepFreeloProjects
              fetchProjects={() => listFreeloProjects(freeloConnectionId)}
              onFinish={freeloFinishProjects}
              onBack={() => setStep("creds")}
            />
            {error && (
              <p className="text-xs text-[var(--danger)]" role="alert">
                {error}
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers / sub-components
// ---------------------------------------------------------------------------

function providerLabel(p: ProviderKind | null): string {
  if (p === "jira") return "Jira";
  if (p === "freelo") return "Freelo";
  return "";
}

function errMsg(e: unknown, fallback: string): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return fallback;
}

function NameField({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <Field
      id="add-conn-name"
      label="Název připojení"
      type="text"
      placeholder="např. SAB, Klient X, …"
      value={value}
      onChange={onChange}
    />
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

function TestStatus({
  test,
}: {
  test:
    | { kind: "idle" }
    | { kind: "loading" }
    | { kind: "ok"; user: ProviderUser | { displayName: string } }
    | { kind: "error"; message: string };
}) {
  if (test.kind === "ok") {
    return (
      <span
        className="flex items-center gap-1.5 text-xs text-[var(--success)]"
        role="status"
      >
        <CircleCheck className="w-4 h-4" aria-hidden />
        Připojeno jako {test.user.displayName}
      </span>
    );
  }
  if (test.kind === "error") {
    return (
      <span className="text-xs text-[var(--danger)]" role="alert">
        {test.message}
      </span>
    );
  }
  return null;
}

function FooterButtons({
  onBack,
  onPrimary,
  primaryLabel,
  primaryDisabled,
  primaryLoading,
  primaryTestId,
}: {
  onBack: () => void;
  onPrimary: () => void;
  primaryLabel: string;
  primaryDisabled: boolean;
  primaryLoading: boolean;
  primaryTestId?: string;
}) {
  return (
    <div className="flex justify-between mt-2">
      <button
        type="button"
        onClick={onBack}
        className="h-9 px-4 rounded-[var(--radius-md)] border border-[var(--border-default)]
                   hover:bg-[var(--bg-hover)] text-sm font-medium text-[var(--text-primary)]
                   transition-colors duration-150"
      >
        Zpět
      </button>
      <button
        type="button"
        onClick={onPrimary}
        disabled={primaryDisabled}
        data-testid={primaryTestId}
        className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)]
                   hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)]
                   disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed
                   text-[var(--accent-text)] text-sm font-medium
                   transition-colors duration-150 flex items-center gap-2"
      >
        {primaryLoading && (
          <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
        )}
        {primaryLabel}
      </button>
    </div>
  );
}


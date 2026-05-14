/**
 * Settings → Connection tab.
 *
 * Read-only display of the current base URL + email, with two actions:
 *   - Edit: in-place form to update base_url + email (optionally token).
 *   - Replace token: separate password input + test connection + save.
 *   - Sign out: clears config + keychain via `sign_out()` and routes to /setup.
 */
import { CheckCircle2, ExternalLink, Loader2, Pencil, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  getCurrentConfig,
  signOut,
  testJiraConnection,
  updateConfig,
} from "../../api/commands";
import type { JiraConfig, JiraUser } from "../../api/types";
import { Button } from "../../components/common/Button";
import { ConfirmButton } from "../../components/common/ConfirmButton";
import { Spinner } from "../../components/common/Spinner";
import { emailSchema, urlSchema } from "../../lib/validation";

type Mode = "view" | "edit" | "token";

interface FormState {
  baseUrl: string;
  email: string;
  token: string;
}

export default function Connection() {
  const navigate = useNavigate();
  const [config, setConfig] = useState<JiraConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [mode, setMode] = useState<Mode>("view");
  const [form, setForm] = useState<FormState>({
    baseUrl: "",
    email: "",
    token: "",
  });
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<JiraUser | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    try {
      const cfg = await getCurrentConfig();
      setConfig(cfg);
      if (cfg) {
        setForm((f) => ({ ...f, baseUrl: cfg.base_url, email: cfg.email, token: "" }));
      }
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to load config");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const startEdit = () => {
    setMode("edit");
    setError(null);
    setTestResult(null);
  };

  const startTokenReplace = () => {
    setMode("token");
    setError(null);
    setTestResult(null);
    setForm((f) => ({ ...f, token: "" }));
  };

  const cancel = () => {
    setMode("view");
    setError(null);
    setTestResult(null);
    setForm({
      baseUrl: config?.base_url ?? "",
      email: config?.email ?? "",
      token: "",
    });
  };

  const handleTest = async () => {
    setError(null);
    setTestResult(null);
    if (!urlSchema.safeParse(form.baseUrl).success) {
      setError("Base URL must be a valid https://… address.");
      return;
    }
    if (!emailSchema.safeParse(form.email).success) {
      setError("Email looks invalid.");
      return;
    }
    if (!form.token) {
      setError("Enter the API token before testing.");
      return;
    }
    setTesting(true);
    try {
      const user = await testJiraConnection(form.baseUrl, form.email, form.token);
      setTestResult(user);
    } catch (e) {
      setError(typeof e === "string" ? e : "Connection failed.");
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    setError(null);
    if (
      !urlSchema.safeParse(form.baseUrl).success ||
      !emailSchema.safeParse(form.email).success
    ) {
      setError("Please enter a valid base URL and email.");
      return;
    }
    setSaving(true);
    try {
      const newCfg: JiraConfig = { base_url: form.baseUrl, email: form.email };
      // Pass token only when we have one (Replace token flow); otherwise null.
      await updateConfig(newCfg, form.token ? form.token : null);
      setMode("view");
      setTestResult(null);
      setForm((f) => ({ ...f, token: "" }));
      await refresh();
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to save changes.");
    } finally {
      setSaving(false);
    }
  };

  const handleSignOut = async () => {
    try {
      await signOut();
      navigate("/setup", { replace: true });
    } catch (e) {
      setError(typeof e === "string" ? e : "Sign-out failed.");
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-[var(--text-tertiary)]">
        <Spinner className="w-4 h-4 mr-2" />
        Loading connection settings…
      </div>
    );
  }

  if (!config) {
    return (
      <div className="text-sm text-[var(--text-secondary)]">
        No connection configured.{" "}
        <button
          type="button"
          onClick={() => navigate("/setup", { replace: true })}
          className="text-[var(--accent)] hover:underline"
        >
          Run setup
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <section>
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-3">
          Jira instance
        </h3>
        {mode === "edit" ? (
          <div className="flex flex-col gap-3">
            <Field label="Base URL">
              <input
                type="url"
                value={form.baseUrl}
                onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
                className={inputCls}
              />
            </Field>
            <Field label="Email">
              <input
                type="email"
                value={form.email}
                onChange={(e) => setForm({ ...form, email: e.target.value })}
                className={inputCls}
              />
            </Field>
            <div className="flex items-center gap-1.5">
              <Button variant="primary" size="sm" onClick={handleSave} disabled={saving}>
                {saving && <Spinner className="w-3.5 h-3.5" />}
                Save changes
              </Button>
              <Button variant="secondary" size="sm" onClick={cancel} disabled={saving}>
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex items-start justify-between gap-3">
            <dl className="flex flex-col gap-2 text-sm">
              <div>
                <dt className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">Base URL</dt>
                <dd className="flex items-center gap-1.5">
                  <span className="text-[var(--text-primary)]">{config.base_url}</span>
                  <ExternalLink className="w-3 h-3 text-[var(--text-tertiary)]" aria-hidden />
                </dd>
              </div>
              <div>
                <dt className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">Email</dt>
                <dd className="text-[var(--text-primary)]">{config.email}</dd>
              </div>
            </dl>
            <Button variant="ghost" size="sm" onClick={startEdit}>
              <Pencil className="w-3.5 h-3.5" aria-hidden />
              Edit
            </Button>
          </div>
        )}
      </section>

      <section>
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-tertiary)] mb-3">
          API token
        </h3>
        {mode === "token" ? (
          <div className="flex flex-col gap-3">
            <Field label="New API token">
              <input
                type="password"
                value={form.token}
                onChange={(e) => setForm({ ...form, token: e.target.value })}
                className={inputCls}
                autoFocus
                placeholder="ATATT3xFfGF0…"
              />
            </Field>
            <div className="flex items-center gap-1.5 flex-wrap">
              <Button variant="secondary" size="sm" onClick={handleTest} disabled={testing}>
                {testing ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden />
                ) : (
                  <RefreshCw className="w-3.5 h-3.5" aria-hidden />
                )}
                Test connection
              </Button>
              <Button variant="primary" size="sm" onClick={handleSave} disabled={saving || !form.token}>
                {saving && <Spinner className="w-3.5 h-3.5" />}
                Save token
              </Button>
              <Button variant="ghost" size="sm" onClick={cancel} disabled={saving}>
                Cancel
              </Button>
            </div>
            {testResult && (
              <div className="text-xs text-[var(--success)] flex items-center gap-1.5">
                <CheckCircle2 className="w-3.5 h-3.5" aria-hidden />
                Authenticated as <strong className="text-[var(--text-primary)]">{testResult.displayName}</strong>
              </div>
            )}
          </div>
        ) : (
          <div className="flex items-start justify-between gap-3">
            <p className="text-sm text-[var(--text-secondary)]">
              Token is stored in the OS keychain and never displayed.
            </p>
            <Button variant="secondary" size="sm" onClick={startTokenReplace}>
              Replace token
            </Button>
          </div>
        )}
      </section>

      {error && (
        <div className="text-xs text-[var(--danger)]" role="alert">
          {error}
        </div>
      )}

      <section className="border-t border-[var(--border-subtle)] pt-4">
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--danger)] mb-3">
          Danger zone
        </h3>
        <div className="flex items-center justify-between gap-3 flex-wrap">
          <p className="text-xs text-[var(--text-secondary)] max-w-sm">
            Sign out clears the saved config and the API token from the OS
            keychain. You can re-run setup afterward.
          </p>
          <ConfirmButton
            label="Sign out"
            confirmLabel="Yes, sign out"
            onConfirm={handleSignOut}
          />
        </div>
      </section>
    </div>
  );
}

const inputCls =
  "px-2.5 h-8 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] " +
  "focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] " +
  "text-sm text-[var(--text-primary)] w-full transition-colors duration-150";

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
        {label}
      </span>
      {children}
    </label>
  );
}

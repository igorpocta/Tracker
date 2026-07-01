import { CircleCheck, LoaderCircle } from "lucide-react";
import { useMemo, useState } from "react";

import { testJiraConnection } from "../../api/commands";
import type { JiraUser } from "../../api/types";
import { useT } from "../../i18n";
import { firstError, tokenSchema } from "../../lib/validation";

export interface StepTokenProps {
  value: string;
  onChange: (next: string) => void;
  /**
   * Called when the user clicks "Finish" *and* the connection test has passed
   * (so credentials are known good).
   */
  onFinish: (user: JiraUser) => void | Promise<void>;
  onBack: () => void;
  baseUrl: string;
  email: string;
}

type TestState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ok"; user: JiraUser }
  | { kind: "error"; message: string };

/**
 * Step 3 — Jira API token + connection test.
 *
 * The "Finish" button only enables once we've successfully called
 * `test_jira_connection` against the supplied credentials. Changing any input
 * invalidates the previous test result.
 */
export function StepToken({
  value,
  onChange,
  onFinish,
  onBack,
  baseUrl,
  email,
}: StepTokenProps) {
  const t = useT();
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [submitting, setSubmitting] = useState(false);

  const shapeError = useMemo(
    () => (value.length === 0 ? null : firstError(tokenSchema, value)),
    [value],
  );
  const hasValidShape = useMemo(
    () => tokenSchema.safeParse(value).success,
    [value],
  );
  const canTest = hasValidShape && test.kind !== "loading";
  const canFinish = test.kind === "ok" && !submitting;

  async function handleTest() {
    setTest({ kind: "loading" });
    try {
      const user = await testJiraConnection(baseUrl, email, value);
      setTest({ kind: "ok", user });
    } catch (e) {
      const message =
        typeof e === "string"
          ? e
          : e instanceof Error
            ? e.message
            : "connection failed";
      setTest({ kind: "error", message });
    }
  }

  async function handleFinish(e: React.FormEvent) {
    e.preventDefault();
    if (test.kind !== "ok" || submitting) return;
    setSubmitting(true);
    try {
      await onFinish(test.user);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleFinish} className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <label htmlFor="setup-token" className="text-sm font-medium text-[var(--text-primary)]">
          {t("setup.token.label")}
        </label>
        <input
          id="setup-token"
          type="password"
          placeholder={t("setup.token.placeholder")}
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
            if (test.kind !== "idle") setTest({ kind: "idle" });
          }}
          aria-invalid={shapeError !== null}
          aria-describedby={shapeError ? "setup-token-error" : undefined}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm font-mono text-[var(--text-primary)] transition-colors duration-150"
        />
        {shapeError && (
          <p id="setup-token-error" className="text-xs text-[var(--danger)]">
            {shapeError}
          </p>
        )}
        <p className="text-xs text-[var(--text-tertiary)]">
          {t("setup.token.hintPrefix")}{" "}
          <span className="text-[var(--text-secondary)]">id.atlassian.com → Security → API tokens</span>.
        </p>
      </div>

      <div className="flex items-center gap-3 flex-wrap">
        <button
          type="button"
          onClick={handleTest}
          disabled={!canTest}
          className="h-9 px-4 rounded-[var(--radius-md)] border border-[var(--border-default)] hover:bg-[var(--bg-hover)] disabled:opacity-40 disabled:cursor-not-allowed text-sm font-medium text-[var(--text-primary)] transition-colors duration-150 flex items-center gap-2"
        >
          {test.kind === "loading" && (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          )}
          {t("setup.button.testConnection")}
        </button>

        {test.kind === "ok" && (
          <span
            className="flex items-center gap-1.5 text-xs text-[var(--success)]"
            role="status"
          >
            <CircleCheck className="w-4 h-4" aria-hidden />
            {t("setup.status.connectedAs", { name: test.user.displayName })}
          </span>
        )}
        {test.kind === "error" && (
          <span className="text-xs text-[var(--danger)]" role="alert">
            {test.message}
          </span>
        )}
      </div>

      <div className="flex justify-between mt-2">
        <button
          type="button"
          onClick={onBack}
          className="h-9 px-4 rounded-[var(--radius-md)] border border-[var(--border-default)] hover:bg-[var(--bg-hover)] text-sm font-medium text-[var(--text-primary)] transition-colors duration-150"
        >
          {t("setup.button.back")}
        </button>
        <button
          type="submit"
          disabled={!canFinish}
          className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed text-[var(--accent-text)] text-sm font-medium transition-colors duration-150 flex items-center gap-2"
        >
          {submitting && (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          )}
          {t("setup.button.finish")}
        </button>
      </div>
    </form>
  );
}

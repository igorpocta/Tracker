import { CircleCheck, LoaderCircle } from "lucide-react";
import { useMemo, useState } from "react";

import { testJiraConnection } from "../../api/commands";
import type { JiraUser } from "../../api/types";
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
        <label htmlFor="setup-token" className="text-sm font-medium">
          Jira API token
        </label>
        <input
          id="setup-token"
          type="password"
          placeholder="paste your token"
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
            // Any edit invalidates the previous test result.
            if (test.kind !== "idle") setTest({ kind: "idle" });
          }}
          aria-invalid={shapeError !== null}
          aria-describedby={shapeError ? "setup-token-error" : undefined}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          className="px-3 py-2 rounded-md bg-neutral-950 border border-neutral-700 focus:border-sky-500 focus:outline-none focus:ring-1 focus:ring-sky-500 text-sm font-mono"
        />
        {shapeError && (
          <p id="setup-token-error" className="text-xs text-red-400">
            {shapeError}
          </p>
        )}
        <p className="text-xs text-neutral-500">
          Create one at{" "}
          <span className="text-neutral-400">id.atlassian.com → Security → API tokens</span>.
        </p>
      </div>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={handleTest}
          disabled={!canTest}
          className="px-4 py-2 rounded-md bg-neutral-800 hover:bg-neutral-700 disabled:bg-neutral-900 disabled:text-neutral-600 disabled:cursor-not-allowed text-sm font-medium transition-colors flex items-center gap-2"
        >
          {test.kind === "loading" && (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          )}
          Test connection
        </button>

        {test.kind === "ok" && (
          <span
            className="flex items-center gap-1.5 text-xs text-emerald-400"
            role="status"
          >
            <CircleCheck className="w-4 h-4" aria-hidden />
            Connected as {test.user.displayName}
          </span>
        )}
        {test.kind === "error" && (
          <span className="text-xs text-red-400" role="alert">
            {test.message}
          </span>
        )}
      </div>

      <div className="flex justify-between mt-2">
        <button
          type="button"
          onClick={onBack}
          className="px-4 py-2 rounded-md bg-neutral-800 hover:bg-neutral-700 text-sm font-medium transition-colors"
        >
          Back
        </button>
        <button
          type="submit"
          disabled={!canFinish}
          className="px-4 py-2 rounded-md bg-emerald-600 hover:bg-emerald-500 disabled:bg-neutral-800 disabled:text-neutral-500 disabled:cursor-not-allowed text-sm font-medium transition-colors flex items-center gap-2"
        >
          {submitting && (
            <LoaderCircle className="w-4 h-4 animate-spin" aria-hidden />
          )}
          Finish
        </button>
      </div>
    </form>
  );
}

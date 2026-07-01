/**
 * Step 2 (Freelo) — email + API key.
 *
 * Freelo uses HTTP Basic auth with the user's email + API key (similar to
 * Jira's email + token). The default base URL `https://api.freelo.io/v1`
 * is filled in automatically; advanced users can override it.
 */
import { CircleCheck, LoaderCircle } from "lucide-react";
import { useMemo, useState } from "react";

import { testConnectionForProvider } from "../../api/commands";
import type { ProviderUser } from "../../api/types";
import { useT } from "../../i18n";
import {
  emailSchema,
  firstError,
  freeloApiKeySchema,
} from "../../lib/validation";

export interface StepFreeloCredsProps {
  email: string;
  apiKey: string;
  baseUrl: string;
  onChangeEmail: (next: string) => void;
  onChangeApiKey: (next: string) => void;
  onChangeBaseUrl: (next: string) => void;
  onTested: (user: ProviderUser) => void;
  onBack: () => void;
}

type TestState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ok"; user: ProviderUser }
  | { kind: "error"; message: string };

export function StepFreeloCreds({
  email,
  apiKey,
  baseUrl,
  onChangeEmail,
  onChangeApiKey,
  onChangeBaseUrl,
  onTested,
  onBack,
}: StepFreeloCredsProps) {
  const t = useT();
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [advanced, setAdvanced] = useState(false);

  const emailError = useMemo(
    () => (email.length === 0 ? null : firstError(emailSchema, email)),
    [email],
  );
  const apiKeyError = useMemo(
    () => (apiKey.length === 0 ? null : firstError(freeloApiKeySchema, apiKey)),
    [apiKey],
  );

  const canTest =
    emailSchema.safeParse(email).success &&
    freeloApiKeySchema.safeParse(apiKey).success &&
    test.kind !== "loading";

  async function handleTest() {
    setTest({ kind: "loading" });
    try {
      const user = await testConnectionForProvider({
        provider: "freelo",
        config: { base_url: baseUrl, email },
        token: apiKey,
      });
      setTest({ kind: "ok", user });
      onTested(user);
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

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (canTest) handleTest();
      }}
      className="flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1.5">
        <label
          htmlFor="freelo-email"
          className="text-sm font-medium text-[var(--text-primary)]"
        >
          {t("setup.freelo.emailLabel")}
        </label>
        <input
          id="freelo-email"
          type="email"
          placeholder="you@example.com"
          value={email}
          onChange={(e) => {
            onChangeEmail(e.target.value);
            if (test.kind !== "idle") setTest({ kind: "idle" });
          }}
          aria-invalid={emailError !== null}
          autoFocus
          autoComplete="email"
          spellCheck={false}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm text-[var(--text-primary)] transition-colors duration-150"
        />
        {emailError && (
          <p className="text-xs text-[var(--danger)]">{emailError}</p>
        )}
      </div>

      <div className="flex flex-col gap-1.5">
        <label
          htmlFor="freelo-api-key"
          className="text-sm font-medium text-[var(--text-primary)]"
        >
          {t("setup.freelo.apiKeyLabel")}
        </label>
        <input
          id="freelo-api-key"
          type="password"
          placeholder={t("setup.freelo.apiKeyPlaceholder")}
          value={apiKey}
          onChange={(e) => {
            onChangeApiKey(e.target.value);
            if (test.kind !== "idle") setTest({ kind: "idle" });
          }}
          aria-invalid={apiKeyError !== null}
          autoComplete="off"
          spellCheck={false}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm font-mono text-[var(--text-primary)] transition-colors duration-150"
        />
        {apiKeyError && (
          <p className="text-xs text-[var(--danger)]">{apiKeyError}</p>
        )}
        <p className="text-xs text-[var(--text-tertiary)]">
          {t("setup.freelo.apiKeyHintPrefix")}{" "}
          <span className="text-[var(--text-secondary)]">
            {t("setup.freelo.apiKeyHintPath")}
          </span>
          .
        </p>
      </div>

      <button
        type="button"
        onClick={() => setAdvanced((a) => !a)}
        className="self-start text-xs text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors duration-150"
      >
        {advanced
          ? t("setup.freelo.advancedHide")
          : t("setup.freelo.advancedShow")}
      </button>

      {advanced && (
        <div className="flex flex-col gap-1.5">
          <label
            htmlFor="freelo-base-url"
            className="text-sm font-medium text-[var(--text-primary)]"
          >
            {t("setup.freelo.baseUrlLabel")}
          </label>
          <input
            id="freelo-base-url"
            type="url"
            placeholder="https://api.freelo.io/v1"
            value={baseUrl}
            onChange={(e) => onChangeBaseUrl(e.target.value)}
            spellCheck={false}
            className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm text-[var(--text-primary)] transition-colors duration-150"
          />
        </div>
      )}

      <div className="flex items-center gap-3 flex-wrap">
        <button
          type="submit"
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
      </div>
    </form>
  );
}

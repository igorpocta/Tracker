import { useMemo } from "react";

import { useT } from "../../i18n";
import { emailSchema, firstError } from "../../lib/validation";

export interface StepEmailProps {
  value: string;
  onChange: (next: string) => void;
  onNext: () => void;
  onBack: () => void;
}

/**
 * Step 2 — Atlassian account email used together with the API token for Basic
 * auth. Basic shape validation only; the real check happens when the user hits
 * "Test connection" in step 3.
 */
export function StepEmail({ value, onChange, onNext, onBack }: StepEmailProps) {
  const t = useT();
  const error = useMemo(
    () => (value.length === 0 ? null : firstError(emailSchema, value)),
    [value],
  );
  const isValid = useMemo(() => emailSchema.safeParse(value).success, [value]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (isValid) onNext();
      }}
      className="flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1.5">
        <label htmlFor="setup-email" className="text-sm font-medium text-[var(--text-primary)]">
          {t("setup.email.label")}
        </label>
        <input
          id="setup-email"
          type="email"
          placeholder="you@example.com"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={error !== null}
          aria-describedby={error ? "setup-email-error" : undefined}
          autoFocus
          autoComplete="email"
          spellCheck={false}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm text-[var(--text-primary)] transition-colors duration-150"
        />
        {error && (
          <p id="setup-email-error" className="text-xs text-[var(--danger)]">
            {error}
          </p>
        )}
        <p className="text-xs text-[var(--text-tertiary)]">
          {t("setup.email.hint")}
        </p>
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
          disabled={!isValid}
          className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed text-[var(--accent-text)] text-sm font-medium transition-colors duration-150"
        >
          {t("setup.button.next")}
        </button>
      </div>
    </form>
  );
}

import { useMemo } from "react";

import { firstError, urlSchema } from "../../lib/validation";

export interface StepUrlProps {
  value: string;
  onChange: (next: string) => void;
  onNext: () => void;
}

/**
 * Step 1 — Jira base URL.
 *
 * Validates that the value is a real URL using `https://`. "Next" stays
 * disabled until validation passes.
 */
export function StepUrl({ value, onChange, onNext }: StepUrlProps) {
  // Don't surface "must be a valid URL" before the user has typed anything.
  const error = useMemo(
    () => (value.length === 0 ? null : firstError(urlSchema, value)),
    [value],
  );
  const isValid = useMemo(() => urlSchema.safeParse(value).success, [value]);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (isValid) onNext();
      }}
      className="flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1.5">
        <label htmlFor="setup-url" className="text-sm font-medium text-[var(--text-primary)]">
          Jira base URL
        </label>
        <input
          id="setup-url"
          type="url"
          placeholder="https://yourorg.atlassian.net"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={error !== null}
          aria-describedby={error ? "setup-url-error" : undefined}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          className="px-3 h-9 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] text-sm text-[var(--text-primary)] transition-colors duration-150"
        />
        {error && (
          <p id="setup-url-error" className="text-xs text-[var(--danger)]">
            {error}
          </p>
        )}
        <p className="text-xs text-[var(--text-tertiary)]">
          Your Atlassian cloud URL, e.g. <code>https://acme.atlassian.net</code>.
        </p>
      </div>

      <div className="flex justify-end mt-2">
        <button
          type="submit"
          disabled={!isValid}
          className="h-9 px-4 rounded-[var(--radius-md)] bg-[var(--accent)] hover:bg-[var(--accent-hover)] disabled:bg-[var(--bg-active)] disabled:text-[var(--text-disabled)] disabled:cursor-not-allowed text-[var(--accent-text)] text-sm font-medium transition-colors duration-150"
        >
          Next
        </button>
      </div>
    </form>
  );
}

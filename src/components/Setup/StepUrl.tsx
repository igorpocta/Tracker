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
        <label htmlFor="setup-url" className="text-sm font-medium">
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
          className="px-3 py-2 rounded-md bg-neutral-950 border border-neutral-700 focus:border-sky-500 focus:outline-none focus:ring-1 focus:ring-sky-500 text-sm"
        />
        {error && (
          <p id="setup-url-error" className="text-xs text-red-400">
            {error}
          </p>
        )}
        <p className="text-xs text-neutral-500">
          Your Atlassian cloud URL, e.g. <code>https://acme.atlassian.net</code>.
        </p>
      </div>

      <div className="flex justify-end mt-2">
        <button
          type="submit"
          disabled={!isValid}
          className="px-4 py-2 rounded-md bg-sky-600 hover:bg-sky-500 disabled:bg-neutral-800 disabled:text-neutral-500 disabled:cursor-not-allowed text-sm font-medium transition-colors"
        >
          Next
        </button>
      </div>
    </form>
  );
}

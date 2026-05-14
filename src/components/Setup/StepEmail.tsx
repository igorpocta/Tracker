import { useMemo } from "react";

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
        <label htmlFor="setup-email" className="text-sm font-medium">
          Atlassian account email
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
          className="px-3 py-2 rounded-md bg-neutral-950 border border-neutral-700 focus:border-sky-500 focus:outline-none focus:ring-1 focus:ring-sky-500 text-sm"
        />
        {error && (
          <p id="setup-email-error" className="text-xs text-red-400">
            {error}
          </p>
        )}
        <p className="text-xs text-neutral-500">
          The email tied to your Atlassian account.
        </p>
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
          disabled={!isValid}
          className="px-4 py-2 rounded-md bg-sky-600 hover:bg-sky-500 disabled:bg-neutral-800 disabled:text-neutral-500 disabled:cursor-not-allowed text-sm font-medium transition-colors"
        >
          Next
        </button>
      </div>
    </form>
  );
}

/**
 * Segmented-style radio group. Renders the options as a row of pill buttons.
 *
 * Each option has a string value; we don't expose the underlying `<input>`
 * because the styling is much simpler when we own the buttons directly.
 * Native radio semantics are simulated via `role="radio"` + `aria-checked`.
 */
import { clsx } from "clsx";
import type { ReactNode } from "react";

export interface RadioOption<T extends string = string> {
  value: T;
  label: ReactNode;
  /** Optional helper text under the label. */
  hint?: ReactNode;
}

export interface RadioGroupProps<T extends string = string> {
  /** Used for the `role="radiogroup"` aria label. */
  label: string;
  name?: string;
  options: RadioOption<T>[];
  value: T;
  onChange: (next: T) => void;
  className?: string;
}

export function RadioGroup<T extends string = string>({
  label,
  options,
  value,
  onChange,
  className,
}: RadioGroupProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={clsx(
        "inline-flex flex-wrap gap-0.5 p-0.5 rounded-[var(--radius-md)] bg-[var(--bg-app)] border border-[var(--border-subtle)]",
        className,
      )}
    >
      {options.map((opt) => {
        const checked = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={checked}
            onClick={() => onChange(opt.value)}
            className={clsx(
              "rounded-[var(--radius-sm)] px-2.5 py-1 text-xs transition-colors duration-150 flex flex-col items-start gap-0.5 text-left",
              checked
                ? "bg-[var(--bg-surface)] text-[var(--text-primary)] shadow-[var(--shadow-sm)]"
                : "text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]",
            )}
          >
            <span className="font-medium">{opt.label}</span>
            {opt.hint && (
              <span
                className={clsx(
                  "text-[10px]",
                  checked ? "text-[var(--text-tertiary)]" : "text-[var(--text-tertiary)]",
                )}
              >
                {opt.hint}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

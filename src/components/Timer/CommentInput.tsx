/**
 * Multi-line textarea used in the Stop dialog. Pulled out as its own
 * component for testability and consistent styling.
 */
import type { ChangeEvent } from "react";

export interface CommentInputProps {
  value: string;
  onChange: (next: string) => void;
  /** Used to wire the textarea to its visible label. */
  id?: string;
  disabled?: boolean;
  placeholder?: string;
}

export function CommentInput({
  value,
  onChange,
  id = "worklog-comment",
  disabled,
  placeholder = "Co jste dělal/a? (volitelné)",
}: CommentInputProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-[var(--text-secondary)]">
        Komentář
      </label>
      <textarea
        id={id}
        value={value}
        onChange={(e: ChangeEvent<HTMLTextAreaElement>) =>
          onChange(e.target.value)
        }
        placeholder={placeholder}
        disabled={disabled}
        rows={3}
        className="px-3 py-2 rounded-[var(--radius-md)]
                   bg-transparent border border-[var(--border-default)]
                   focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                   text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]
                   resize-y min-h-[72px] max-h-[200px] disabled:opacity-50 transition-colors duration-150"
      />
    </div>
  );
}

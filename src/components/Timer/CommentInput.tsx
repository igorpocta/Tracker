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
  placeholder = "What did you do? (optional)",
}: CommentInputProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-neutral-300">
        Comment
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
        className="px-3 py-2 rounded-md bg-neutral-950 border border-neutral-800 focus:border-sky-500 focus:outline-none focus:ring-1 focus:ring-sky-500 text-sm resize-y min-h-[72px] max-h-[200px] disabled:opacity-50"
      />
    </div>
  );
}

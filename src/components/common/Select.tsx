/**
 * Native `<select>` wrapped with our app styling.
 *
 * Building a custom listbox is rarely worth the accessibility work — the
 * OS-native dropdown reads better and integrates with platform conventions.
 */
import { clsx } from "clsx";
import { ChevronDown } from "lucide-react";
import type { SelectHTMLAttributes } from "react";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps
  extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> {
  options: SelectOption[];
}

export function Select({ options, className, ...rest }: SelectProps) {
  return (
    <div className={clsx("relative inline-block", className)}>
      <select
        {...rest}
        className={clsx(
          "appearance-none pl-2.5 pr-7 h-8 rounded-[var(--radius-md)]",
          "bg-transparent border border-[var(--border-default)] text-xs text-[var(--text-primary)]",
          "focus:outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent-ring)]",
          "cursor-pointer transition-colors duration-150",
        )}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown
        aria-hidden
        className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--text-tertiary)] pointer-events-none"
      />
    </div>
  );
}

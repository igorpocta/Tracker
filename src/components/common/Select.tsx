/**
 * Native `<select>` wrapped with our app styling.
 *
 * For Tracker MVP we deliberately use the OS-native dropdown — building a
 * custom listbox with full keyboard support is a yak we don't need shaving.
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
          "appearance-none pl-2.5 pr-7 py-1.5 rounded-md bg-neutral-950 border border-neutral-800",
          "text-xs text-neutral-100 focus:outline-none focus:border-sky-500 focus:ring-1 focus:ring-sky-500",
          "cursor-pointer",
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
        className="absolute right-1.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-neutral-500 pointer-events-none"
      />
    </div>
  );
}

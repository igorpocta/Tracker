/**
 * Search input with a search-icon affordance and a clear button.
 *
 * Debouncing happens at the call site (`useQuery` keys recompute on every
 * keystroke but actual IPC is debounced via `useEffect`).
 */
import { Search, X } from "lucide-react";

export interface SearchInputProps {
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
}

export function SearchInput({
  value,
  onChange,
  placeholder = "Search issues…",
}: SearchInputProps) {
  return (
    <div className="relative">
      <Search
        className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--text-tertiary)] pointer-events-none"
        aria-hidden
      />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label="Search issues"
        className="w-full pl-8 pr-8 h-9 rounded-[var(--radius-md)]
                   bg-transparent border border-[var(--border-default)]
                   focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                   text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)]
                   transition-colors duration-150"
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          aria-label="Clear search"
          className="absolute right-2 top-1/2 -translate-y-1/2 w-5 h-5 inline-flex items-center justify-center rounded
                     text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
        >
          <X className="w-3 h-3" aria-hidden />
        </button>
      )}
    </div>
  );
}

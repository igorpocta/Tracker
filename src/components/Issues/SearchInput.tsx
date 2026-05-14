/**
 * Sidebar search input with a search-icon affordance and a clear button.
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
        className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-neutral-500 pointer-events-none"
        aria-hidden
      />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label="Search issues"
        className="w-full pl-7 pr-7 py-1.5 rounded-md bg-neutral-950 border border-neutral-800 focus:border-sky-500 focus:outline-none focus:ring-1 focus:ring-sky-500 text-xs"
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          aria-label="Clear search"
          className="absolute right-1.5 top-1/2 -translate-y-1/2 w-5 h-5 inline-flex items-center justify-center rounded text-neutral-400 hover:text-white hover:bg-neutral-800"
        >
          <X className="w-3 h-3" aria-hidden />
        </button>
      )}
    </div>
  );
}

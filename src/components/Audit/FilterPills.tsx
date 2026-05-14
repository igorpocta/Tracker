/**
 * Multi-select filter pills for the Historie změn route.
 *
 *   [ Vše ] [ Smazáno ] [ Změněno ] [ Selhalo ]
 *
 * "Vše" is exclusive — clicking it clears the rest. The other three are
 * additive (Smazáno + Změněno can both be on). "Selhalo" combines with the
 * op filters in the natural way (server-side AND).
 *
 * Token-only colors.
 */
import { clsx } from "clsx";

export type FilterKey = "all" | "delete" | "update" | "failed";

export interface FilterPillsProps {
  /** Set of currently-active filter keys. */
  active: Set<FilterKey>;
  onChange: (next: Set<FilterKey>) => void;
}

const PILLS: { key: FilterKey; label: string }[] = [
  { key: "all", label: "Vše" },
  { key: "delete", label: "Smazáno" },
  { key: "update", label: "Změněno" },
  { key: "failed", label: "Selhalo" },
];

export function FilterPills({ active, onChange }: FilterPillsProps) {
  const handleClick = (key: FilterKey) => {
    if (key === "all") {
      onChange(new Set(["all"]));
      return;
    }
    const next = new Set(active);
    next.delete("all");
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    // If nothing left, snap back to "all".
    if (next.size === 0) next.add("all");
    onChange(next);
  };

  return (
    <div
      role="group"
      aria-label="Filtr historie"
      className="inline-flex items-center gap-1 p-1 rounded-[var(--radius-md)]"
      style={{ background: "var(--bg-surface)", border: "1px solid var(--border-subtle)" }}
    >
      {PILLS.map((p) => {
        const isActive = active.has(p.key);
        return (
          <button
            key={p.key}
            type="button"
            aria-pressed={isActive}
            onClick={() => handleClick(p.key)}
            className={clsx(
              "h-6 px-2.5 rounded-[var(--radius-sm)] text-[11px] font-medium",
              "transition-colors duration-150",
            )}
            style={
              isActive
                ? {
                    background: "var(--accent-soft)",
                    color: "var(--accent)",
                  }
                : {
                    background: "transparent",
                    color: "var(--text-secondary)",
                  }
            }
          >
            {p.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Minimal tab strip + content container.
 *
 * Controlled component — the parent owns the active tab id. The strip
 * renders horizontally; active tab gets an accent underline (not a fill).
 */
import { clsx } from "clsx";
import type { ReactNode } from "react";

export interface TabDef {
  id: string;
  label: ReactNode;
  /** Optional leading icon. */
  icon?: ReactNode;
}

export interface TabsProps {
  tabs: TabDef[];
  active: string;
  onChange: (id: string) => void;
  className?: string;
}

export function Tabs({ tabs, active, onChange, className }: TabsProps) {
  return (
    <div
      role="tablist"
      className={clsx(
        "flex items-center gap-1 border-b border-[var(--border-subtle)]",
        className,
      )}
    >
      {tabs.map((t) => (
        <button
          key={t.id}
          type="button"
          role="tab"
          aria-selected={active === t.id}
          aria-controls={`tabpanel-${t.id}`}
          id={`tab-${t.id}`}
          tabIndex={active === t.id ? 0 : -1}
          onClick={() => onChange(t.id)}
          onKeyDown={(e) => handleTabKey(e, tabs, active, onChange)}
          className={clsx(
            "inline-flex items-center gap-1.5 px-3 py-2 text-xs border-b-2 -mb-[1px] transition-colors duration-150",
            active === t.id
              ? "border-[var(--accent)] text-[var(--text-primary)]"
              : "border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)]",
          )}
        >
          {t.icon}
          {t.label}
        </button>
      ))}
    </div>
  );
}

function handleTabKey(
  e: React.KeyboardEvent<HTMLButtonElement>,
  tabs: TabDef[],
  active: string,
  onChange: (id: string) => void,
) {
  if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
  e.preventDefault();
  const idx = tabs.findIndex((t) => t.id === active);
  if (idx < 0) return;
  const next =
    e.key === "ArrowRight"
      ? tabs[(idx + 1) % tabs.length]
      : tabs[(idx - 1 + tabs.length) % tabs.length];
  onChange(next.id);
}

export interface TabPanelProps {
  /** Must match the `id` of the corresponding `TabDef`. */
  id: string;
  active: string;
  children: ReactNode;
  className?: string;
}

export function TabPanel({ id, active, children, className }: TabPanelProps) {
  if (id !== active) return null;
  return (
    <div
      role="tabpanel"
      id={`tabpanel-${id}`}
      aria-labelledby={`tab-${id}`}
      className={className}
    >
      {children}
    </div>
  );
}

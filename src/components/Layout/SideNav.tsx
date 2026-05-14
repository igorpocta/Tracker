/**
 * Vertical macOS-style icon rail along the left edge of the shell.
 *
 * Active route is signalled by an accent-soft background + accent text.
 * Hover reveals the label via a CSS tooltip (no popover lib needed).
 */
import { clsx } from "clsx";
import { BarChart3, CalendarDays, Settings as SettingsIcon, Sun } from "lucide-react";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";

export interface NavItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Today", icon: <Sun className="w-4 h-4" aria-hidden />, end: true },
  { to: "/history", label: "History", icon: <CalendarDays className="w-4 h-4" aria-hidden /> },
  { to: "/reports", label: "Reports", icon: <BarChart3 className="w-4 h-4" aria-hidden /> },
  { to: "/settings", label: "Settings", icon: <SettingsIcon className="w-4 h-4" aria-hidden /> },
];

export function SideNav() {
  return (
    <nav
      aria-label="Primary"
      className="flex flex-col items-stretch gap-0.5 px-1.5 py-3 w-[64px] shrink-0
                 border-r border-[var(--border-subtle)] bg-[var(--bg-surface)]/60"
    >
      {NAV_ITEMS.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          className={({ isActive }) =>
            clsx(
              "group relative flex flex-col items-center gap-1 px-1 py-2 rounded-[var(--radius-md)]",
              "transition-colors duration-150",
              isActive
                ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                : "text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]",
            )
          }
        >
          {item.icon}
          <span className="text-[10px] leading-none tracking-tight font-medium">
            {item.label}
          </span>
        </NavLink>
      ))}
    </nav>
  );
}

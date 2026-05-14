/**
 * Vertical icon navigation rail along the left edge of the shell.
 *
 * Renders the four primary routes as icon buttons with a tooltip-ish label
 * appearing on hover (CSS only — no popover library). Active route is
 * indicated by a sky accent + an accent stripe on the left.
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
      className="flex flex-col items-stretch gap-1 px-1.5 py-3 w-[68px] shrink-0 border-r border-neutral-800/70 bg-neutral-950/40"
    >
      {NAV_ITEMS.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          className={({ isActive }) =>
            clsx(
              "group relative flex flex-col items-center gap-1 px-1 py-2 rounded-lg transition-colors",
              isActive
                ? "bg-sky-600/15 text-sky-200"
                : "text-neutral-400 hover:text-white hover:bg-neutral-800/60",
            )
          }
        >
          {({ isActive }) => (
            <>
              {isActive && (
                <span
                  aria-hidden
                  className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-6 rounded-r-full bg-sky-400"
                />
              )}
              {item.icon}
              <span className="text-[10px] leading-none tracking-tight">
                {item.label}
              </span>
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}

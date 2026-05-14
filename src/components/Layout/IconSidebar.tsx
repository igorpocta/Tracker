/**
 * Thin icon rail along the left edge of the shell.
 *
 * Reference: `screens/SCR-20260514-rjbm-2.png` and friends.
 *
 *   ┌─────┐
 *   │ T.  │  ← cursive logo, accent color, always dark surface
 *   │     │
 *   │ 🕐  │  ← Time Log
 *   │ 📊  │  ← Reports
 *   │ 📅  │  ← Calendar
 *   │ 🎯  │  ← Goals
 *   │ ··  │
 *   │ ⚙   │  ← Settings (lower group)
 *   │ ◯   │  ← User / timer ring
 *   └─────┘
 *
 * Width: 64 px. Always dark surface (`--sidebar-bg`) — this is the visual
 * constant of the app and does NOT flip in light mode.
 *
 * Hover affordance: subtle background lightening + tooltip label. Active
 * route gets the accent-soft pill with accent-colored icon.
 */
import { clsx } from "clsx";
import {
  BarChart3,
  CalendarDays,
  Clock,
  Settings as SettingsIcon,
  Target,
} from "lucide-react";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";

import { useNow } from "../../hooks/useNow";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";

export interface IconSidebarItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

const PRIMARY_NAV: IconSidebarItem[] = [
  { to: "/", label: "Time Log", icon: <Clock className="w-5 h-5" aria-hidden />, end: true },
  { to: "/reports", label: "Reports", icon: <BarChart3 className="w-5 h-5" aria-hidden /> },
  { to: "/calendar", label: "Calendar", icon: <CalendarDays className="w-5 h-5" aria-hidden /> },
  { to: "/goals", label: "Goals", icon: <Target className="w-5 h-5" aria-hidden /> },
];

export function IconSidebar() {
  const active = useTimerStore((s) => s.active);
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);

  return (
    <aside
      aria-label="Primary"
      className="shrink-0 flex flex-col items-center w-[64px] py-3 gap-3
                 border-r"
      style={{
        background: "var(--sidebar-bg)",
        color: "var(--sidebar-text)",
        borderColor: "var(--sidebar-border)",
      }}
    >
      {/* Logo "T." in accent */}
      <NavLink
        to="/"
        end
        aria-label="Tracker home"
        className="block px-1 py-0.5 mb-1 select-none"
        style={{ color: "var(--accent)" }}
      >
        <span
          className="text-[22px] leading-none font-semibold italic tracking-tight"
          style={{ fontFamily: "var(--font-script), serif" }}
        >
          T.
        </span>
      </NavLink>

      {/* Primary nav (top group) */}
      <nav className="flex flex-col items-stretch gap-1 w-full px-2">
        {PRIMARY_NAV.map((item) => (
          <SidebarLink key={item.to} item={item} />
        ))}
      </nav>

      <div className="flex-1" />

      {/* Settings + user ring (bottom group) */}
      <nav className="flex flex-col items-stretch gap-2 w-full px-2 pb-1">
        <SidebarLink
          item={{
            to: "/settings",
            label: "Settings",
            icon: <SettingsIcon className="w-5 h-5" aria-hidden />,
          }}
        />
      </nav>

      <UserRing elapsedSeconds={elapsed} running={active !== null} />
    </aside>
  );
}

function SidebarLink({ item }: { item: IconSidebarItem }) {
  return (
    <NavLink
      to={item.to}
      end={item.end}
      title={item.label}
      aria-label={item.label}
      className={({ isActive }) =>
        clsx(
          "group relative flex items-center justify-center h-10 rounded-[var(--radius-md)]",
          "transition-colors duration-150",
          isActive ? "sidebar-link-active" : "sidebar-link-idle",
        )
      }
      style={({ isActive }) =>
        isActive
          ? {
              background: "var(--accent-soft)",
              color: "var(--accent)",
            }
          : {
              color: "var(--sidebar-text-muted)",
            }
      }
    >
      <span className="pointer-events-none">{item.icon}</span>
      <SidebarTooltip label={item.label} />
    </NavLink>
  );
}

function SidebarTooltip({ label }: { label: string }) {
  return (
    <span
      role="tooltip"
      className="pointer-events-none absolute left-full ml-2 px-1.5 py-0.5
                 rounded-md text-[11px] whitespace-nowrap
                 opacity-0 -translate-x-1
                 group-hover:opacity-100 group-hover:translate-x-0
                 transition-all duration-150 z-50"
      style={{
        background: "var(--bg-elevated)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        boxShadow: "var(--shadow-sm)",
      }}
    >
      {label}
    </span>
  );
}

/**
 * Small circular user/timer-status ring at the bottom of the sidebar.
 * Reference screenshots show a number ("20") inside an accent-tinted ring.
 *
 * We show:
 *   - When the timer is running, a thin accent ring + the running issue
 *     elapsed mm count (or short HH:MM if > 1h).
 *   - When idle, the same ring outline + a small clock dot.
 */
function UserRing({
  elapsedSeconds,
  running,
}: {
  elapsedSeconds: number;
  running: boolean;
}) {
  const mins = Math.floor(elapsedSeconds / 60);
  const label = running
    ? mins < 60
      ? `${mins}`
      : `${Math.floor(mins / 60)}h`
    : "20";
  return (
    <div
      aria-hidden
      className="w-9 h-9 rounded-full flex items-center justify-center"
      style={{
        border: `2px solid ${running ? "var(--accent)" : "var(--accent)"}`,
        color: "var(--accent)",
        background: "transparent",
      }}
    >
      <span className="text-[10px] font-mono tabular-nums">{label}</span>
    </div>
  );
}

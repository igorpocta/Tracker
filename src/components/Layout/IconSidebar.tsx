/**
 * Thin icon rail along the left edge of the shell.
 *
 *   ┌─────┐
 *   │ T.  │  ← cursive logo, accent color, always dark surface
 *   │ 🕐  │  ← Časový záznam
 *   │ 📊  │  ← Reporty
 *   │ 📅  │  ← Kalendář
 *   │ 🎯  │  ← Cíle
 *   │ ··  │
 *   │ ⚙   │  ← Nastavení (lower group)
 *   │ ◯   │  ← Cache count / running ring
 *   └─────┘
 *
 * The bottom ring shows the real cached-issue count (was a hardcoded "20"
 * subscription-limit placeholder before Phase 14). When a timer is running,
 * it shows running minutes instead. Caps at "500+" to keep the chip small;
 * shows "–" when nothing is cached.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { clsx } from "clsx";
import {
  BarChart3,
  CalendarDays,
  Clock,
  History,
  LayoutDashboard,
  Loader2,
  Settings as SettingsIcon,
  Target,
} from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { NavLink } from "react-router-dom";

import {
  getCacheStats,
  getSyncErrors,
  listConnections,
  refreshAll,
} from "../../api/commands";
import { useNow } from "../../hooks/useNow";
import { useTauriEvent } from "../../hooks/useTauriEvent";
import { elapsedSeconds, useTimerStore } from "../../stores/timerStore";

export interface IconSidebarItem {
  to: string;
  label: string;
  icon: ReactNode;
  end?: boolean;
}

const PRIMARY_NAV_BASE: IconSidebarItem[] = [
  { to: "/", label: "Časový záznam", icon: <Clock className="w-5 h-5" aria-hidden />, end: true },
  { to: "/reports", label: "Reporty", icon: <BarChart3 className="w-5 h-5" aria-hidden /> },
  { to: "/calendar", label: "Kalendář", icon: <CalendarDays className="w-5 h-5" aria-hidden /> },
  { to: "/goals", label: "Cíle", icon: <Target className="w-5 h-5" aria-hidden /> },
  {
    to: "/audit",
    label: "Historie změn",
    icon: <History className="w-5 h-5" aria-hidden />,
  },
];

const JIRA_DASHBOARD_NAV: IconSidebarItem = {
  to: "/jira-dashboard",
  label: "JIRA Přehled",
  icon: <LayoutDashboard className="w-5 h-5" aria-hidden />,
};

export function IconSidebar() {
  const active = useTimerStore((s) => s.active);
  const now = useNow(active ? 1000 : 60_000);
  const elapsed = elapsedSeconds(active, now);
  const queryClient = useQueryClient();
  const [syncing, setSyncing] = useState(false);

  const statsQ = useQuery({
    queryKey: ["cache-stats"],
    queryFn: getCacheStats,
    staleTime: 30_000,
  });

  // "JIRA Přehled" menu položka se zobrazí jen když existuje aspoň jedna
  // Jira connection s `dashboard_enabled = true`. Bez toho by tlačítko
  // vedlo na prázdnou stránku.
  const connectionsQ = useQuery({
    queryKey: ["connections"],
    queryFn: listConnections,
    staleTime: 60_000,
  });
  const hasDashboardConnection = (connectionsQ.data ?? []).some(
    (c) =>
      c.provider === "jira" &&
      c.enabled &&
      (c.config as Record<string, unknown>)?.["dashboard_enabled"] === true,
  );
  const primaryNav: IconSidebarItem[] = hasDashboardConnection
    ? [...PRIMARY_NAV_BASE, JIRA_DASHBOARD_NAV]
    : PRIMARY_NAV_BASE;

  // Persistovaný stav posledně neúspěšného syncu — pokud je seznam neprázdný,
  // ring se přebarví červeně a v tooltipu vidíš co padlo. Vyčistí se hned, jak
  // další sync téhož připojení proběhne OK.
  //
  // `staleTime: 0` + `refetchInterval` zajistí, že po vyčištění chyby v DB
  // (např. úspěšným auto-syncem na pozadí) UI rychle dohoní stav, i kdyby
  // se invalidace přes `auto-sync-complete` event minula s React Query cache.
  const syncErrorsQ = useQuery({
    queryKey: ["sync-errors"],
    queryFn: getSyncErrors,
    staleTime: 0,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
  });

  useTauriEvent<unknown>("cache-refreshed", () => {
    queryClient.invalidateQueries({ queryKey: ["cache-stats"] });
  });
  useTauriEvent<unknown>("auto-sync-progress", () => {
    setSyncing(true);
  });
  useTauriEvent<unknown>("auto-sync-complete", () => {
    setSyncing(false);
    queryClient.invalidateQueries({ queryKey: ["cache-stats"] });
    queryClient.invalidateQueries({ queryKey: ["worklogs-range"] });
    queryClient.invalidateQueries({ queryKey: ["sync-errors"] });
    queryClient.invalidateQueries({ queryKey: ["connections"] });
  });
  useEffect(() => {
    /* hook deps satisfied; statsQ kept implicit */
  }, []);

  const cachedIssues = statsQ.data?.issues ?? 0;

  const handleRefresh = async () => {
    if (syncing) return;
    setSyncing(true);
    try {
      // Sidebar = rychlé tlačítko pro běžný refresh. Plnou historii lze
      // vynutit v Nastavení → konkrétní integrace → „Stáhnout celou historii".
      await refreshAll("incremental");
    } catch {
      // Errors surface via the SyncBanner / worklog-error toasts elsewhere;
      // we just need to release the local flag so the user can retry.
    } finally {
      // Backend emits auto-sync-complete which will flip this off too — this
      // is a safety net in case the command rejects before any event fires.
      setSyncing(false);
    }
  };

  return (
    <aside
      aria-label="Hlavní navigace"
      className="shrink-0 flex flex-col items-center w-[64px] py-3 gap-3
                 border-r"
      style={{
        background: "var(--sidebar-bg)",
        color: "var(--sidebar-text)",
        borderColor: "var(--sidebar-border)",
      }}
    >
      {/*
        Logo — minimalist stopwatch, same shape as the app icon but stroked
        in the active accent color over a transparent background. `currentColor`
        propagates from the parent NavLink's inline `color: var(--accent)`,
        so it follows palette changes instantly.
      */}
      <NavLink
        to="/"
        end
        aria-label="Tracker — domů"
        className="block px-1 py-0.5 mb-1 select-none"
        style={{ color: "var(--accent)" }}
      >
        <svg
          viewBox="0 0 24 24"
          width="26"
          height="26"
          fill="none"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
        >
          {/* Winding stem above the ring at 12 o'clock. */}
          <rect
            x="10.4"
            y="2"
            width="3.2"
            height="2"
            rx="0.7"
            fill="currentColor"
            stroke="none"
          />
          {/* Stopwatch ring. */}
          <circle cx="12" cy="14" r="8" strokeWidth="1.9" />
          {/* Clock hand at ~11 o'clock — sin(-60°)*5 ≈ -4.33, cos(-60°)*5 = 2.5. */}
          <line x1="12" y1="14" x2="7.67" y2="11.5" strokeWidth="1.9" />
          {/* Centre cap. */}
          <circle cx="12" cy="14" r="0.9" fill="currentColor" stroke="none" />
        </svg>
      </NavLink>

      {/* Primary nav (top group) */}
      <nav className="flex flex-col items-stretch gap-1 w-full px-2">
        {primaryNav.map((item) => (
          <SidebarLink key={item.to} item={item} />
        ))}
      </nav>

      <div className="flex-1" />

      {/* Settings + user ring (bottom group) */}
      <nav className="flex flex-col items-stretch gap-2 w-full px-2 pb-1">
        <SidebarLink
          item={{
            to: "/settings",
            label: "Nastavení",
            icon: <SettingsIcon className="w-5 h-5" aria-hidden />,
          }}
        />
      </nav>

      <CacheRing
        elapsedSeconds={elapsed}
        running={active !== null}
        cachedIssues={cachedIssues}
        syncing={syncing}
        syncErrors={syncErrorsQ.data ?? []}
        onRefresh={handleRefresh}
      />
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
 * Bottom ring chip — počet úkolů v cache. Kliknutí spustí kompletní sync
 * (úkoly + worklogy ze všech connections). Když běží timer, ring místo
 * počtu ukazuje uplynulé minuty, ale klik pořád funguje jako refresh.
 *
 * Když je `syncErrors` neprázdné, ring se přebarví na danger barvu a
 * tooltip ukazuje, co padlo — vizuálně neuteče, že poslední sync selhal.
 */
function CacheRing({
  elapsedSeconds,
  running,
  cachedIssues,
  syncing,
  syncErrors,
  onRefresh,
}: {
  elapsedSeconds: number;
  running: boolean;
  cachedIssues: number;
  syncing: boolean;
  syncErrors: import("../../api/commands").SyncErrorEntry[];
  onRefresh: () => void;
}) {
  let label: string;
  if (running) {
    const mins = Math.floor(elapsedSeconds / 60);
    label = mins < 60 ? `${mins}` : `${Math.floor(mins / 60)}h`;
  } else {
    label = formatCacheCount(cachedIssues);
  }
  const hasError = syncErrors.length > 0;
  const errorTitle = hasError
    ? syncErrors
        .map((e) => `#${e.connection_id} ${e.phase}: ${e.error}`)
        .join("\n")
    : null;
  const title = syncing
    ? "Synchronizace probíhá…"
    : hasError
      ? `Poslední sync selhal · klikni pro nový pokus\n\n${errorTitle}`
      : running
        ? `Sledování běží · klikni pro sync (${cachedIssues} úkolů v cache)`
        : `${cachedIssues} úkolů v cache · klikni pro sync`;
  const ringColor = hasError ? "var(--danger, #c0392b)" : "var(--accent)";
  // Inline styly mají vyšší specificitu než pseudo-classy v Tailwind preflight,
  // ale `:focus` a `:disabled` v UA stylech přebíjí třídy, ne inline. Proto
  // všechno barevné držíme inline — accent přežije focus, hover i disabled.
  // Hover/active state simulujeme přes lokální `hovered` state, ne přes CSS,
  // ať se vyhnem CSS variable resolveru, který někdy laguje při paletovém
  // přepnutí.
  const [hovered, setHovered] = useState(false);
  const [pressed, setPressed] = useState(false);
  const background = hovered
    ? "color-mix(in srgb, " + ringColor + " 14%, transparent)"
    : "transparent";
  const scale = pressed ? 0.95 : hovered ? 1.05 : 1;
  return (
    <button
      type="button"
      onClick={onRefresh}
      disabled={syncing}
      aria-label={title}
      title={title}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        setHovered(false);
        setPressed(false);
      }}
      onMouseDown={() => setPressed(true)}
      onMouseUp={() => setPressed(false)}
      className={clsx(
        "w-9 h-9 rounded-full flex items-center justify-center",
        "transition-all duration-150 cursor-pointer appearance-none",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-1",
        "disabled:cursor-progress",
        syncing && "animate-pulse",
      )}
      style={{
        border: `2px solid ${ringColor}`,
        color: ringColor,
        background,
        transform: `scale(${scale})`,
        // Drží accent i při disabled — bez tohoto UA aplikuje `graytext`.
        opacity: syncing ? 0.75 : 1,
      }}
    >
      {syncing ? (
        <Loader2 className="w-4 h-4 animate-spin" aria-hidden />
      ) : (
        <span className="text-[10px] font-mono tabular-nums" style={{ color: ringColor }}>
          {label}
        </span>
      )}
    </button>
  );
}

/**
 * Phase 18B — Item 5: compact cache-count formatting.
 *
 *   0           → "–"
 *   1 – 999     → exact (e.g. "42", "999")
 *   1000–1999   → "1k+"
 *   2000–9999   → "2k+", "3k+", …, "9k+"
 *   10000+      → "10k+"
 */
export function formatCacheCount(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "–";
  if (n < 1000) return `${Math.floor(n)}`;
  if (n >= 10000) return "10k+";
  return `${Math.floor(n / 1000)}k+`;
}

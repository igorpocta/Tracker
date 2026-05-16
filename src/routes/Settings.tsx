/**
 * Settings route — internal sidebar + content pane.
 *
 * Internal nav (left), in order:
 *   • Connection (URL, email, replace token, sign out)
 *   • General (day timeline toggle, time input style, auto re-index interval)
 *   • Reporting (hourly rate, currency)
 *   • Goals (daily hours goal slider)
 *   • Appearance (theme + palette)
 *   • Extensions (placeholder)
 *
 * No Profile, no Subscription — this is a local-only internal app. The
 * active tab is stored in the URL as `?tab=` so deep-links and back/forward
 * navigation behave correctly.
 */
import { clsx } from "clsx";
import { useSearchParams } from "react-router-dom";

import About from "../components/Settings/About";
import Appearance from "../components/Settings/Appearance";
import Connection from "../components/Settings/Connection";
import General from "../components/Settings/General";
import Reporting from "../components/Settings/Reporting";
import SettingsGoals from "../components/Settings/SettingsGoals";

type TabId =
  | "connection"
  | "general"
  | "reporting"
  | "goals"
  | "appearance"
  | "about";

const TABS: { id: TabId; label: string }[] = [
  { id: "connection", label: "Připojení" },
  { id: "general", label: "Obecné" },
  { id: "reporting", label: "Reporting" },
  { id: "goals", label: "Cíle" },
  { id: "appearance", label: "Vzhled" },
  { id: "about", label: "O aplikaci" },
];

const TAB_IDS = new Set<TabId>(TABS.map((t) => t.id));

export default function Settings() {
  const [params, setParams] = useSearchParams();
  const raw = params.get("tab") ?? "connection";
  const active: TabId = TAB_IDS.has(raw as TabId) ? (raw as TabId) : "connection";

  const setActive = (id: TabId) => {
    const next = new URLSearchParams(params);
    next.set("tab", id);
    setParams(next, { replace: true });
  };

  return (
    <div className="flex w-full h-full">
      {/* Internal sidebar ------------------------------------------------ */}
      <nav
        aria-label="Sekce nastavení"
        className="w-[220px] shrink-0 px-3 py-5 border-r border-[var(--border-subtle)]
                   flex flex-col gap-0.5"
      >
        <h2 className="text-sm font-semibold text-[var(--text-primary)] px-2 mb-2">
          Nastavení
        </h2>
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setActive(t.id)}
            className={clsx(
              "text-left px-2 h-8 rounded-[var(--radius-md)] text-sm",
              "transition-colors duration-150",
              active === t.id
                ? "bg-[var(--bg-active)] text-[var(--text-primary)]"
                : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]",
            )}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {/* Content pane ---------------------------------------------------- */}
      <main className="flex-1 min-w-0 overflow-y-auto px-8 py-5">
        {active === "connection" && <Connection />}
        {active === "general" && <General />}
        {active === "reporting" && <Reporting />}
        {active === "goals" && <SettingsGoals />}
        {active === "appearance" && <Appearance />}
        {active === "about" && <About />}
      </main>
    </div>
  );
}

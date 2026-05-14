/**
 * Settings route — internal sidebar + content pane.
 *
 * Reference: every `screens/SCR-20260514-rj{gv|hq|iy|jq|ks|mh}-2.png`.
 *
 * Internal nav (left):
 *   • Profile
 *   • General
 *   • Reporting
 *   • Goals
 *   • Appearance
 *   • Extensions
 *   • Connection Setup
 *
 * Active tab is stored in the URL as `?tab=` so deep-links and back/forward
 * navigation behave correctly.
 */
import { clsx } from "clsx";
import { useSearchParams } from "react-router-dom";

import Appearance from "../components/Settings/Appearance";
import Connection from "../components/Settings/Connection";
import Extensions from "../components/Settings/Extensions";
import General from "../components/Settings/General";
import Profile from "../components/Settings/Profile";
import Reporting from "../components/Settings/Reporting";
import SettingsGoals from "../components/Settings/SettingsGoals";

type TabId =
  | "profile"
  | "general"
  | "reporting"
  | "goals"
  | "appearance"
  | "extensions"
  | "connection";

const TABS: { id: TabId; label: string }[] = [
  { id: "profile", label: "Profile" },
  { id: "general", label: "General" },
  { id: "reporting", label: "Reporting" },
  { id: "goals", label: "Goals" },
  { id: "appearance", label: "Appearance" },
  { id: "extensions", label: "Extensions" },
  { id: "connection", label: "Connection Setup" },
];

const TAB_IDS = new Set<TabId>(TABS.map((t) => t.id));

export default function Settings() {
  const [params, setParams] = useSearchParams();
  const raw = params.get("tab") ?? "profile";
  const active: TabId = TAB_IDS.has(raw as TabId) ? (raw as TabId) : "profile";

  const setActive = (id: TabId) => {
    const next = new URLSearchParams(params);
    next.set("tab", id);
    setParams(next, { replace: true });
  };

  return (
    <div className="flex w-full h-full">
      {/* Internal sidebar ------------------------------------------------ */}
      <nav
        aria-label="Settings sections"
        className="w-[220px] shrink-0 px-3 py-5 border-r border-[var(--border-subtle)]
                   flex flex-col gap-0.5"
      >
        <h2 className="text-sm font-semibold text-[var(--text-primary)] px-2 mb-2">
          Settings
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
        {active === "profile" && <Profile />}
        {active === "general" && <General />}
        {active === "reporting" && <Reporting />}
        {active === "goals" && <SettingsGoals />}
        {active === "appearance" && <Appearance />}
        {active === "extensions" && <Extensions />}
        {active === "connection" && <Connection />}
      </main>
    </div>
  );
}

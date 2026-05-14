/**
 * Settings route — tabbed shell with four sections.
 *
 * The active tab is persisted to the URL via `?tab=` so deep-links from the
 * tray menu (or backend events) can land directly on a specific section.
 */
import { useSearchParams } from "react-router-dom";

import { Card } from "../components/common/Card";
import { Tabs, TabPanel } from "../components/common/Tabs";
import About from "./Settings/About";
import Appearance from "./Settings/Appearance";
import Connection from "./Settings/Connection";
import TimeSettings from "./Settings/Time";

const TABS = [
  { id: "connection", label: "Connection" },
  { id: "appearance", label: "Appearance" },
  { id: "time", label: "Time" },
  { id: "about", label: "About" },
];

const VALID_IDS = new Set(TABS.map((t) => t.id));

export default function Settings() {
  const [params, setParams] = useSearchParams();
  const raw = params.get("tab") ?? "connection";
  const active = VALID_IDS.has(raw) ? raw : "connection";

  const setActive = (id: string) => {
    const next = new URLSearchParams(params);
    next.set("tab", id);
    setParams(next, { replace: true });
  };

  return (
    <div className="p-6 flex flex-col gap-4 max-w-4xl mx-auto w-full">
      <header>
        <h1 className="text-lg font-semibold">Settings</h1>
        <p className="text-xs text-neutral-500 mt-0.5">
          Connection details, appearance, time, and app info.
        </p>
      </header>

      <Card padding="none">
        <div className="px-2 pt-2">
          <Tabs tabs={TABS} active={active} onChange={setActive} />
        </div>
        <div className="p-5">
          <TabPanel id="connection" active={active}>
            <Connection />
          </TabPanel>
          <TabPanel id="appearance" active={active}>
            <Appearance />
          </TabPanel>
          <TabPanel id="time" active={active}>
            <TimeSettings />
          </TabPanel>
          <TabPanel id="about" active={active}>
            <About />
          </TabPanel>
        </div>
      </Card>
    </div>
  );
}

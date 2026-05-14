/**
 * Settings → General.
 *
 * Reference: `screens/SCR-20260514-rjhq-2.png`.
 *
 *   Day timeline   [ Visible ●● ] [ Hidden ── ]
 *   Time input style   ( ) End time  ( ) Duration
 *   Auto re-index interval   [ Every hour ▾ ]
 *
 * The day-timeline toggle and time-input style are local UI prefs persisted
 * via `localStorage` (no backend roundtrip required for personal opinions
 * that don't need to sync across windows). The re-index interval is the
 * standard pref.
 */
import { useEffect, useState } from "react";

const LS_TIMELINE_KEY = "tracker.dayTimeline";
const LS_TIME_INPUT_KEY = "tracker.timeInput";
const LS_REINDEX_KEY = "tracker.reindexInterval";

export type TimelineVisibility = "visible" | "hidden";
export type TimeInputStyle = "end" | "duration";
export type ReindexInterval = "manual" | "15m" | "1h" | "4h" | "daily";

const REINDEX_LABEL: Record<ReindexInterval, string> = {
  manual: "Manual only",
  "15m": "Every 15 minutes",
  "1h": "Every hour",
  "4h": "Every 4 hours",
  daily: "Once a day",
};

export default function General() {
  const [timeline, setTimeline] = useState<TimelineVisibility>("visible");
  const [timeInput, setTimeInput] = useState<TimeInputStyle>("end");
  const [reindex, setReindex] = useState<ReindexInterval>("1h");

  useEffect(() => {
    try {
      const tl = window.localStorage.getItem(LS_TIMELINE_KEY);
      if (tl === "visible" || tl === "hidden") setTimeline(tl);
      const ti = window.localStorage.getItem(LS_TIME_INPUT_KEY);
      if (ti === "end" || ti === "duration") setTimeInput(ti);
      const r = window.localStorage.getItem(LS_REINDEX_KEY);
      if (r === "manual" || r === "15m" || r === "1h" || r === "4h" || r === "daily") {
        setReindex(r as ReindexInterval);
      }
    } catch {
      /* ignore */
    }
  }, []);

  const updateTimeline = (v: TimelineVisibility) => {
    setTimeline(v);
    try {
      window.localStorage.setItem(LS_TIMELINE_KEY, v);
    } catch {
      /* ignore */
    }
  };
  const updateTimeInput = (v: TimeInputStyle) => {
    setTimeInput(v);
    try {
      window.localStorage.setItem(LS_TIME_INPUT_KEY, v);
    } catch {
      /* ignore */
    }
  };
  const updateReindex = (v: ReindexInterval) => {
    setReindex(v);
    try {
      window.localStorage.setItem(LS_REINDEX_KEY, v);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="flex flex-col gap-8 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          General
        </h2>
      </header>

      <Section
        title="Day timeline"
        description="Show or hide the visual day timeline above the entry rows."
      >
        <div className="grid grid-cols-2 gap-3">
          <ToggleCard
            label="Visible"
            active={timeline === "visible"}
            onClick={() => updateTimeline("visible")}
          >
            <TimelinePreview filled />
          </ToggleCard>
          <ToggleCard
            label="Hidden"
            active={timeline === "hidden"}
            onClick={() => updateTimeline("hidden")}
          >
            <TimelinePreview filled={false} />
          </ToggleCard>
        </div>
      </Section>

      <Section
        title="Time input style"
        description="When adding an entry, choose whether you prefer to set an end time or a duration."
      >
        <div className="flex flex-col gap-2">
          <RadioRow
            label="End time — pick when the work ended"
            checked={timeInput === "end"}
            onChange={() => updateTimeInput("end")}
          />
          <RadioRow
            label="Duration — enter the number of minutes"
            checked={timeInput === "duration"}
            onChange={() => updateTimeInput("duration")}
          />
        </div>
      </Section>

      <Section
        title="Auto re-index interval"
        description="How often Jira issues are automatically re-indexed in the background."
      >
        <select
          value={reindex}
          onChange={(e) => updateReindex(e.target.value as ReindexInterval)}
          className="w-full h-9 px-3 rounded-[var(--radius-md)] bg-transparent
                     border border-[var(--border-subtle)] text-sm
                     text-[var(--text-primary)] focus:outline-none
                     focus:border-[var(--border-default)] transition-colors duration-150"
        >
          {(Object.keys(REINDEX_LABEL) as ReindexInterval[]).map((k) => (
            <option key={k} value={k}>
              {REINDEX_LABEL[k]}
            </option>
          ))}
        </select>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          You can also re-index anytime by clicking the index icon in the sidebar
          or pressing{" "}
          <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘I</kbd>.
        </p>
      </Section>
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h3 className="text-sm font-semibold text-[var(--text-primary)]">{title}</h3>
      {description && (
        <p className="text-[11px] text-[var(--text-tertiary)] mt-0.5 mb-3">
          {description}
        </p>
      )}
      {children}
    </section>
  );
}

function ToggleCard({
  label,
  active,
  onClick,
  children,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-[var(--radius-lg)] p-4 text-left transition-colors duration-150
                 border"
      style={{
        background: active ? "var(--accent-soft)" : "var(--bg-surface)",
        borderColor: active ? "var(--accent)" : "var(--border-subtle)",
      }}
    >
      <div className="h-12 mb-3 flex items-center">{children}</div>
      <div
        className="text-xs font-medium"
        style={{ color: active ? "var(--accent)" : "var(--text-primary)" }}
      >
        {label}
      </div>
    </button>
  );
}

function TimelinePreview({ filled }: { filled: boolean }) {
  return (
    <div className="w-full flex items-end gap-1 h-full">
      {Array.from({ length: 6 }).map((_, i) => (
        <div
          key={i}
          className="flex-1 rounded-sm"
          style={{
            height: filled ? `${30 + (i * 8) % 50}%` : "8%",
            background: filled
              ? "var(--accent)"
              : "var(--border-default)",
            opacity: filled ? 0.7 + (i % 3) * 0.1 : 0.5,
          }}
        />
      ))}
    </div>
  );
}

function RadioRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <label className="flex items-center gap-2 cursor-pointer text-sm text-[var(--text-secondary)]">
      <input
        type="radio"
        checked={checked}
        onChange={onChange}
        className="appearance-none w-4 h-4 rounded-full border
                   border-[var(--border-default)] checked:bg-[var(--accent)]
                   checked:border-[var(--accent)] relative
                   transition-colors duration-150"
      />
      {label}
    </label>
  );
}

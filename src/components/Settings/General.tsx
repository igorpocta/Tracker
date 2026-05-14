/**
 * Settings → Obecné (General).
 *
 *   Časová osa dne     [ Viditelná ●● ] [ Skrytá ── ]
 *   Styl zadávání času ( ) Koncový čas  ( ) Trvání
 *   Interval automatické re-indexace [ Každou hodinu ▾ ]
 *
 * Day-timeline visibility is now backend-backed (Phase 14) and read through
 * the prefs store; this directly drives whether `<DayTimeline>` renders on
 * the Time Log route. The other two opinions remain local-only in
 * `localStorage` until we have a real need to sync them across windows.
 */
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import { purgeAuditLog } from "../../api/commands";
import { usePrefsStore } from "../../stores/prefsStore";
import { Button } from "../common/Button";
import { ConfirmButton } from "../common/ConfirmButton";

const LS_TIME_INPUT_KEY = "tracker.timeInput";
const LS_REINDEX_KEY = "tracker.reindexInterval";

export type TimeInputStyle = "end" | "duration";
export type ReindexInterval = "manual" | "15m" | "1h" | "4h" | "daily";

const REINDEX_LABEL: Record<ReindexInterval, string> = {
  manual: "Pouze ručně",
  "15m": "Každých 15 minut",
  "1h": "Každou hodinu",
  "4h": "Každé 4 hodiny",
  daily: "Jednou denně",
};

export default function General() {
  const dayTimelineVisible = usePrefsStore((s) => s.dayTimelineVisible);
  const setDayTimelineVisible = usePrefsStore((s) => s.setDayTimelineVisible);
  const navigate = useNavigate();

  const [timeInput, setTimeInput] = useState<TimeInputStyle>("end");
  const [reindex, setReindex] = useState<ReindexInterval>("1h");
  const [purgeDays, setPurgeDays] = useState(90);
  const [purgeStatus, setPurgeStatus] = useState<string | null>(null);

  useEffect(() => {
    try {
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
          Obecné
        </h2>
      </header>

      <Section
        title="Časová osa dne"
        description="Zobrazit nebo skrýt vizuální časovou osu nad záznamy."
      >
        <div className="grid grid-cols-2 gap-3">
          <ToggleCard
            label="Viditelná"
            active={dayTimelineVisible}
            onClick={() => void setDayTimelineVisible(true)}
          >
            <TimelinePreview filled />
          </ToggleCard>
          <ToggleCard
            label="Skrytá"
            active={!dayTimelineVisible}
            onClick={() => void setDayTimelineVisible(false)}
          >
            <TimelinePreview filled={false} />
          </ToggleCard>
        </div>
      </Section>

      <Section
        title="Styl zadávání času"
        description="Při přidávání záznamu zvolte, jestli preferujete nastavit koncový čas nebo trvání."
      >
        <div className="flex flex-col gap-2">
          <RadioRow
            label="Koncový čas — vyberte, kdy práce skončila"
            checked={timeInput === "end"}
            onChange={() => updateTimeInput("end")}
          />
          <RadioRow
            label="Trvání — zadejte počet minut"
            checked={timeInput === "duration"}
            onChange={() => updateTimeInput("duration")}
          />
        </div>
      </Section>

      <Section
        title="Interval automatické re-indexace"
        description="Jak často se na pozadí automaticky reindexují úkoly z Jiry."
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
          Reindexovat můžete také kdykoli ručně kliknutím na ikonu v liště nebo
          stisknutím{" "}
          <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘I</kbd>.
        </p>
      </Section>

      <Section
        title="Historie změn"
        description="Každá akce s worklogem (vytvoření, úprava, smazání, přesun) se ukládá do lokální historie. Z historie lze obnovit smazaný záznam zpět do Jiry nebo vrátit nedávnou úpravu."
      >
        <div className="flex flex-col gap-3">
          <div>
            <Button
              variant="secondary"
              size="md"
              onClick={() => navigate("/audit")}
            >
              Otevřít historii změn
            </Button>
          </div>
          <div className="flex items-end gap-2 flex-wrap">
            <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
              <span>Vyprázdnit starší než</span>
              <div className="flex items-center gap-1">
                <input
                  type="number"
                  min={1}
                  max={3650}
                  value={purgeDays}
                  onChange={(e) =>
                    setPurgeDays(Math.max(1, Number(e.target.value) || 1))
                  }
                  className="w-24 h-8 px-2 rounded-[var(--radius-md)] bg-transparent
                             border border-[var(--border-subtle)] text-sm
                             text-[var(--text-primary)] focus:outline-none
                             focus:border-[var(--border-default)]
                             transition-colors duration-150 tabular-nums"
                />
                <span className="text-xs text-[var(--text-tertiary)]">dní</span>
              </div>
            </label>
            <ConfirmButton
              label="Vyčistit"
              confirmLabel="Vyčistit"
              variant="danger"
              onConfirm={async () => {
                try {
                  const n = await purgeAuditLog(purgeDays);
                  setPurgeStatus(`Smazáno ${n} záznam${czechPlural(n)}.`);
                } catch (e) {
                  setPurgeStatus(
                    typeof e === "string" ? e : "Vyčištění selhalo",
                  );
                }
              }}
            />
          </div>
          {purgeStatus && (
            <p className="text-[11px] text-[var(--text-tertiary)]">
              {purgeStatus}
            </p>
          )}
          <p className="text-[11px] text-[var(--text-tertiary)]">
            Smaže audit záznamy starší než zvolený počet dní. Tuto akci nelze
            vrátit.
          </p>
        </div>
      </Section>
    </div>
  );
}

/** Czech plural ending for "záznam(y/ů)" based on count. */
function czechPlural(n: number): string {
  if (n === 1) return "";
  if (n >= 2 && n <= 4) return "y";
  return "ů";
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

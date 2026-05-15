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
import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  exportBackup,
  getActivityThresholdMin,
  getAutostart,
  getInstallId,
  getRoundingIntervalMinutes,
  getRoundingMode,
  getSentryEnabled,
  importBackup,
  purgeAuditLog,
  setActivityThresholdMin,
  setAutostart,
  setRoundingIntervalMinutes,
  setRoundingMode,
  setSentryEnabled,
  type RoundingMode,
} from "../../api/commands";
import {
  activityThresholdSchema,
  firstError,
  roundingIntervalSchema,
} from "../../lib/validation";
import { initSentry, shutdownSentry } from "../../lib/sentry";
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

  // Phase 18A — Item 30: autostart on login (opt-in).
  const [autostartOn, setAutostartOn] = useState<boolean | null>(null);
  // Phase 18A — Item 27: time rounding.
  const [rndMode, setRndMode] = useState<RoundingMode>("none");
  const [rndInterval, setRndInterval] = useState<number>(1);
  // Phase 18A — Item 32: inactivity threshold (minutes).
  const [actThreshold, setActThreshold] = useState<number>(5);
  // Phase 19: anonymous error reporting.
  const [sentryOn, setSentryOn] = useState<boolean | null>(null);

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

  // Hydrate Phase 18A toggles from the backend.
  useEffect(() => {
    void getAutostart().then(setAutostartOn).catch(() => setAutostartOn(false));
    void getRoundingMode().then(setRndMode).catch(() => {});
    void getRoundingIntervalMinutes().then(setRndInterval).catch(() => {});
    void getActivityThresholdMin().then(setActThreshold).catch(() => {});
    void getSentryEnabled().then(setSentryOn).catch(() => setSentryOn(false));
  }, []);

  const updateAutostart = async (enabled: boolean) => {
    setAutostartOn(enabled);
    try {
      await setAutostart(enabled);
    } catch {
      // Revert on failure (e.g. permission denied).
      setAutostartOn(!enabled);
    }
  };

  const updateRndMode = async (m: RoundingMode) => {
    setRndMode(m);
    await setRoundingMode(m).catch(() => {});
  };
  const updateRndInterval = async (n: number) => {
    // Validate before submit — silently clamp to the allowed enum.
    if (firstError(roundingIntervalSchema, n)) return;
    setRndInterval(n);
    await setRoundingIntervalMinutes(n).catch(() => {});
  };
  const updateActThreshold = async (n: number) => {
    // Already clamped by the input handler, but double-check the schema in
    // case a future caller bypasses the input.
    if (firstError(activityThresholdSchema, n)) return;
    setActThreshold(n);
    await setActivityThresholdMin(n).catch(() => {});
  };

  // Phase 19: when the user opts in we initialise the frontend SDK
  // immediately (so the current session is covered, not just the next
  // restart). When they opt out we flush + close.
  const updateSentry = async (enabled: boolean) => {
    const previous = sentryOn;
    setSentryOn(enabled);
    try {
      await setSentryEnabled(enabled);
      if (enabled) {
        const installId = await getInstallId().catch(() => null);
        initSentry({ installId });
      } else {
        await shutdownSentry();
      }
    } catch {
      // Revert on failure.
      setSentryOn(previous);
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
    <div className="flex flex-col gap-8 w-full">
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
        title="Zaokrouhlování času"
        description="Před uložením do Jiry můžete dobu zaokrouhlit nahoru nebo dolů na zvolený interval."
      >
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-2">
            <RadioRow
              label="Žádné — uložit přesnou dobu"
              checked={rndMode === "none"}
              onChange={() => void updateRndMode("none")}
            />
            <RadioRow
              label="Nahoru — zaokrouhlit na další interval"
              checked={rndMode === "up"}
              onChange={() => void updateRndMode("up")}
            />
            <RadioRow
              label="Dolů — zaokrouhlit na předchozí interval"
              checked={rndMode === "down"}
              onChange={() => void updateRndMode("down")}
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
            <span>Interval:</span>
            <select
              value={rndInterval}
              onChange={(e) => void updateRndInterval(Number(e.target.value))}
              disabled={rndMode === "none"}
              className="h-8 px-2 rounded-[var(--radius-md)] bg-transparent
                         border border-[var(--border-subtle)] text-sm
                         text-[var(--text-primary)] focus:outline-none
                         focus:border-[var(--border-default)] transition-colors duration-150
                         disabled:opacity-50"
            >
              <option value={1}>1 minuta</option>
              <option value={5}>5 minut</option>
              <option value={15}>15 minut</option>
              <option value={60}>1 hodina</option>
            </select>
          </label>
        </div>
      </Section>

      <Section
        title="Spustit při přihlášení"
        description="Tracker se automaticky spustí při přihlášení do systému. Okno zůstane skryté — bude dostupné z menu baru."
      >
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={autostartOn === true}
            onChange={(e) => void updateAutostart(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-[var(--text-primary)]">
            Spouštět Tracker automaticky
          </span>
        </label>
      </Section>

      <Section
        title="Anonymní reportování chyb"
        description="Pokud zapnete, aplikace zasílá anonymizovaná hlášení chyb na Sentry — pomáhá nám diagnostikovat pády. API tokeny, hesla ani plné e-maily se neposílají. Identifikace je pouze anonymním instalačním ID."
      >
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={sentryOn === true}
            onChange={(e) => void updateSentry(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-[var(--text-primary)]">
            Povolit reportování chyb
          </span>
        </label>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          Změna se na frontendu projeví ihned. Backend přejde do nového
          režimu při příštím spuštění aplikace.
        </p>
      </Section>

      <Section
        title="Sledování aktivity"
        description="Tracker sleduje, kdy s aplikací aktivně pracujete, a tuto informaci zobrazuje v přehledu cílů. Nemá vliv na uložené worklogy."
      >
        <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
          <span>Práh nečinnosti (minuty)</span>
          <input
            type="number"
            min={1}
            max={120}
            value={actThreshold}
            onChange={(e) =>
              void updateActThreshold(
                Math.min(120, Math.max(1, Number(e.target.value) || 5)),
              )
            }
            className="w-28 h-8 px-2 rounded-[var(--radius-md)] bg-transparent
                       border border-[var(--border-subtle)] text-sm
                       text-[var(--text-primary)] focus:outline-none
                       focus:border-[var(--border-default)] transition-colors duration-150 tabular-nums"
          />
        </label>
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

      <BackupSection />
    </div>
  );
}

function BackupSection() {
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);

  const handleExport = async () => {
    setStatus(null);
    setBusy(true);
    try {
      const bundle = await exportBackup();
      const json = JSON.stringify(bundle, null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      const ts = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
      a.href = url;
      a.download = `tracker-backup-${ts}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setStatus("Export hotov.");
    } catch (e) {
      setStatus(`Export selhal: ${typeof e === "string" ? e : String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async (file: File) => {
    if (
      !window.confirm(
        `Importovat „${file.name}"?\n\n` +
          `POZOR: existující data v aplikaci budou přepsána. Pokračovat?`,
      )
    ) {
      return;
    }
    setStatus(null);
    setBusy(true);
    try {
      const text = await file.text();
      const bundle = JSON.parse(text);
      const stats = await importBackup(bundle);
      setStatus(
        `Importováno: ${stats.worklogs} worklog(s), ${stats.issues_v2} úkol(s), ${stats.connections} připojení.`,
      );
    } catch (e) {
      setStatus(`Import selhal: ${typeof e === "string" ? e : String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section
      title="Záloha a obnova"
      description="Export všech lokálních dat (worklogy, úkoly, nastavení) do JSON. Tokeny se neukládají — po obnově je nutné znovu zadat. Import přepíše stávající data."
    >
      <div className="flex flex-col gap-2 max-w-md">
        <div className="flex items-center gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={busy}
            className="inline-flex items-center gap-1.5 px-3 h-8 rounded-[var(--radius-md)]
                       text-xs text-[var(--accent)] border border-[var(--accent-soft)]
                       bg-transparent hover:bg-[var(--accent-soft)]
                       transition-colors duration-150 disabled:opacity-60"
          >
            Stáhnout zálohu (.json)
          </button>
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            disabled={busy}
            className="inline-flex items-center gap-1.5 px-3 h-8 rounded-[var(--radius-md)]
                       text-xs text-[var(--text-primary)] border border-[var(--border-default)]
                       hover:bg-[var(--bg-hover)] transition-colors duration-150
                       disabled:opacity-60"
          >
            Obnovit ze souboru…
          </button>
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void handleImport(file);
              // Reset value, ať jde vybrat ten samý soubor znovu po pokusu.
              e.currentTarget.value = "";
            }}
          />
        </div>
        {status && (
          <p className="text-[11px] text-[var(--text-tertiary)]">{status}</p>
        )}
      </div>
    </Section>
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

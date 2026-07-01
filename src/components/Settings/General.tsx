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
import { useNavigate, useOutletContext } from "react-router-dom";

import {
  exportBackup,
  getActivityThresholdMin,
  getAutoSyncIntervalSeconds,
  getAutostart,
  getInstallId,
  getRoundingIntervalMinutes,
  getRoundingMode,
  getSentryEnabled,
  getSmartSuggestionsEnabled,
  importBackup,
  purgeAuditLog,
  setActivityThresholdMin,
  setAutoSyncIntervalSeconds,
  setAutostart,
  setRoundingIntervalMinutes,
  setRoundingMode,
  setSentryEnabled,
  setSmartSuggestionsEnabled,
  type RoundingMode,
} from "../../api/commands";
import {
  activityThresholdSchema,
  firstError,
  roundingIntervalSchema,
} from "../../lib/validation";
import { initSentry, shutdownSentry } from "../../lib/sentry";
import { useT } from "../../i18n";
import { usePrefsStore } from "../../stores/prefsStore";
import type { ShellOutletContext } from "../Layout/AppShell";
import { Button } from "../common/Button";
import { ConfirmButton } from "../common/ConfirmButton";
import { GlobalShortcutSetting } from "./GlobalShortcutSetting";
import { SettingsCard } from "./SettingsCard";

const LS_TIME_INPUT_KEY = "tracker.timeInput";

export type TimeInputStyle = "end" | "duration";
export type ReindexInterval = "manual" | "15m" | "1h" | "4h" | "daily";

const REINDEX_LABEL_KEY: Record<ReindexInterval, string> = {
  manual: "settingsGeneral.reindex.manual",
  "15m": "settingsGeneral.reindex.15m",
  "1h": "settingsGeneral.reindex.1h",
  "4h": "settingsGeneral.reindex.4h",
  daily: "settingsGeneral.reindex.daily",
};

const REINDEX_TO_SECONDS: Record<ReindexInterval, number> = {
  manual: 0,
  "15m": 15 * 60,
  "1h": 60 * 60,
  "4h": 4 * 60 * 60,
  daily: 24 * 60 * 60,
};

function secondsToReindex(seconds: number): ReindexInterval {
  switch (seconds) {
    case 0:
      return "manual";
    case 15 * 60:
      return "15m";
    case 4 * 60 * 60:
      return "4h";
    case 24 * 60 * 60:
      return "daily";
    case 60 * 60:
    default:
      return "1h";
  }
}

export default function General() {
  const t = useT();
  const dayTimelineVisible = usePrefsStore((s) => s.dayTimelineVisible);
  const setDayTimelineVisible = usePrefsStore((s) => s.setDayTimelineVisible);
  const navigate = useNavigate();
  const { pushToast } = useOutletContext<ShellOutletContext>();

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
  // Smart suggestion banner on Time Log ("Jako včera?"). Default true.
  const [smartSuggestionsOn, setSmartSuggestionsOn] = useState<boolean | null>(null);

  useEffect(() => {
    try {
      const ti = window.localStorage.getItem(LS_TIME_INPUT_KEY);
      if (ti === "end" || ti === "duration") setTimeInput(ti);
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
    void getAutoSyncIntervalSeconds()
      .then((s) => setReindex(secondsToReindex(s)))
      .catch(() => {});
    void getSmartSuggestionsEnabled()
      .then(setSmartSuggestionsOn)
      .catch(() => setSmartSuggestionsOn(true));
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
    const previous = rndMode;
    setRndMode(m);
    try {
      await setRoundingMode(m);
    } catch {
      setRndMode(previous);
      pushToast("error", t("settingsGeneral.rounding.saveModeError"));
    }
  };
  const updateRndInterval = async (n: number) => {
    // Validate before submit — silently clamp to the allowed enum.
    if (firstError(roundingIntervalSchema, n)) return;
    const previous = rndInterval;
    setRndInterval(n);
    try {
      await setRoundingIntervalMinutes(n);
    } catch {
      setRndInterval(previous);
      pushToast("error", t("settingsGeneral.rounding.saveIntervalError"));
    }
  };
  const updateActThreshold = async (n: number) => {
    // Already clamped by the input handler, but double-check the schema in
    // case a future caller bypasses the input.
    if (firstError(activityThresholdSchema, n)) return;
    const previous = actThreshold;
    setActThreshold(n);
    try {
      await setActivityThresholdMin(n);
    } catch {
      setActThreshold(previous);
      pushToast("error", t("settingsGeneral.activity.saveError"));
    }
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
    const previous = reindex;
    setReindex(v);
    setAutoSyncIntervalSeconds(REINDEX_TO_SECONDS[v]).catch(() => {
      // Revert on backend failure so the UI doesn't lie about what's persisted.
      setReindex(previous);
    });
  };

  const updateSmartSuggestions = (enabled: boolean) => {
    const previous = smartSuggestionsOn;
    setSmartSuggestionsOn(enabled);
    setSmartSuggestionsEnabled(enabled).catch(() => {
      setSmartSuggestionsOn(previous);
    });
  };

  return (
    <div className="flex flex-col gap-5 w-full max-w-3xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {t("settingsGeneral.heading")}
        </h2>
      </header>

      <Section
        title={t("settingsGeneral.dayTimeline.title")}
        description={t("settingsGeneral.dayTimeline.description")}
      >
        <div className="grid grid-cols-2 gap-3">
          <ToggleCard
            label={t("settingsGeneral.dayTimeline.visible")}
            active={dayTimelineVisible}
            onClick={() => void setDayTimelineVisible(true)}
          >
            <TimelinePreview filled />
          </ToggleCard>
          <ToggleCard
            label={t("settingsGeneral.dayTimeline.hidden")}
            active={!dayTimelineVisible}
            onClick={() => void setDayTimelineVisible(false)}
          >
            <TimelinePreview filled={false} />
          </ToggleCard>
        </div>
      </Section>

      <Section
        title={t("settingsGeneral.timeInput.title")}
        description={t("settingsGeneral.timeInput.description")}
      >
        <div className="flex flex-col gap-2">
          <RadioRow
            label={t("settingsGeneral.timeInput.end")}
            checked={timeInput === "end"}
            onChange={() => updateTimeInput("end")}
          />
          <RadioRow
            label={t("settingsGeneral.timeInput.duration")}
            checked={timeInput === "duration"}
            onChange={() => updateTimeInput("duration")}
          />
        </div>
      </Section>

      <Section
        title={t("settingsGeneral.rounding.title")}
        description={t("settingsGeneral.rounding.description")}
      >
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-2">
            <RadioRow
              label={t("settingsGeneral.rounding.none")}
              checked={rndMode === "none"}
              onChange={() => void updateRndMode("none")}
            />
            <RadioRow
              label={t("settingsGeneral.rounding.up")}
              checked={rndMode === "up"}
              onChange={() => void updateRndMode("up")}
            />
            <RadioRow
              label={t("settingsGeneral.rounding.down")}
              checked={rndMode === "down"}
              onChange={() => void updateRndMode("down")}
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
            <span>{t("settingsGeneral.rounding.intervalLabel")}</span>
            <select
              value={rndInterval}
              onChange={(e) => void updateRndInterval(Number(e.target.value))}
              disabled={rndMode === "none"}
              className="ui-select"
            >
              <option value={1}>{t("settingsGeneral.rounding.interval1")}</option>
              <option value={5}>{t("settingsGeneral.rounding.interval5")}</option>
              <option value={15}>{t("settingsGeneral.rounding.interval15")}</option>
              <option value={60}>{t("settingsGeneral.rounding.interval60")}</option>
            </select>
          </label>
        </div>
      </Section>

      <Section
        title={t("settingsGeneral.shortcut.title")}
        description={t("settingsGeneral.shortcut.description")}
      >
        <GlobalShortcutSetting pushToast={pushToast} />
      </Section>

      <Section
        title={t("settingsGeneral.autostart.title")}
        description={t("settingsGeneral.autostart.description")}
      >
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={autostartOn === true}
            onChange={(e) => void updateAutostart(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-[var(--text-primary)]">
            {t("settingsGeneral.autostart.toggle")}
          </span>
        </label>
      </Section>

      <Section
        title={t("settingsGeneral.smartSuggestions.title")}
        description={t("settingsGeneral.smartSuggestions.description")}
      >
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={smartSuggestionsOn === true}
            onChange={(e) => updateSmartSuggestions(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-[var(--text-primary)]">
            {t("settingsGeneral.smartSuggestions.toggle")}
          </span>
        </label>
      </Section>

      <Section
        title={t("settingsGeneral.sentry.title")}
        description={t("settingsGeneral.sentry.description")}
      >
        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={sentryOn === true}
            onChange={(e) => void updateSentry(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-[var(--text-primary)]">
            {t("settingsGeneral.sentry.toggle")}
          </span>
        </label>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          {t("settingsGeneral.sentry.note")}
        </p>
      </Section>

      <Section
        title={t("settingsGeneral.activity.title")}
        description={t("settingsGeneral.activity.description")}
      >
        <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
          <span>{t("settingsGeneral.activity.thresholdLabel")}</span>
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
            className="ui-input w-28 tabular-nums"
          />
        </label>
      </Section>

      <Section
        title={t("settingsGeneral.reindex.title")}
        description={t("settingsGeneral.reindex.description")}
      >
        <select
          value={reindex}
          onChange={(e) => updateReindex(e.target.value as ReindexInterval)}
          className="ui-select w-full"
        >
          {(Object.keys(REINDEX_LABEL_KEY) as ReindexInterval[]).map((k) => (
            <option key={k} value={k}>
              {t(REINDEX_LABEL_KEY[k])}
            </option>
          ))}
        </select>
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          {t("settingsGeneral.reindex.notePrefix")}
          <kbd className="font-mono px-1 rounded bg-[var(--bg-hover)]">⌘I</kbd>.
        </p>
      </Section>

      <Section
        title={t("settingsGeneral.audit.title")}
        description={t("settingsGeneral.audit.description")}
      >
        <div className="flex flex-col gap-3">
          <div>
            <Button
              variant="secondary"
              size="md"
              onClick={() => navigate("/audit")}
            >
              {t("settingsGeneral.audit.open")}
            </Button>
          </div>
          <div className="flex items-end gap-2 flex-wrap">
            <label className="flex flex-col gap-1 text-xs text-[var(--text-secondary)]">
              <span>{t("settingsGeneral.audit.purgeLabel")}</span>
              <div className="flex items-center gap-1">
                <input
                  type="number"
                  min={1}
                  max={3650}
                  value={purgeDays}
                  onChange={(e) =>
                    setPurgeDays(Math.max(1, Number(e.target.value) || 1))
                  }
                  className="ui-input w-24 tabular-nums"
                />
                <span className="text-xs text-[var(--text-tertiary)]">
                  {t("settingsGeneral.audit.days")}
                </span>
              </div>
            </label>
            <ConfirmButton
              label={t("settingsGeneral.audit.purgeButton")}
              confirmLabel={t("settingsGeneral.audit.purgeConfirm")}
              variant="danger"
              onConfirm={async () => {
                try {
                  const n = await purgeAuditLog(purgeDays);
                  setPurgeStatus(t(purgeDoneKey(n), { n }));
                } catch (e) {
                  setPurgeStatus(
                    typeof e === "string"
                      ? e
                      : t("settingsGeneral.audit.purgeFailed"),
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
            {t("settingsGeneral.audit.purgeHint")}
          </p>
        </div>
      </Section>

      <BackupSection />
    </div>
  );
}

function BackupSection() {
  const t = useT();
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
      setStatus(t("settingsGeneral.backup.exportDone"));
    } catch (e) {
      setStatus(
        t("settingsGeneral.backup.exportFailed", {
          error: typeof e === "string" ? e : String(e),
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async (file: File) => {
    if (
      !window.confirm(
        t("settingsGeneral.backup.importConfirm", { name: file.name }),
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
        t("settingsGeneral.backup.importDone", {
          worklogs: stats.worklogs,
          issues: stats.issues_v2,
          connections: stats.connections,
        }),
      );
    } catch (e) {
      setStatus(
        t("settingsGeneral.backup.importFailed", {
          error: typeof e === "string" ? e : String(e),
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section
      title={t("settingsGeneral.backup.title")}
      description={t("settingsGeneral.backup.description")}
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
            {t("settingsGeneral.backup.download")}
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
            {t("settingsGeneral.backup.restore")}
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

/** i18n key for the purge-done message based on Czech plural rules. */
function purgeDoneKey(n: number): string {
  if (n === 1) return "settingsGeneral.audit.purgeDone.one";
  if (n >= 2 && n <= 4) return "settingsGeneral.audit.purgeDone.few";
  return "settingsGeneral.audit.purgeDone.many";
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
    <SettingsCard title={title} description={description}>
      {children}
    </SettingsCard>
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
        background: active ? "var(--accent-soft)" : "var(--bg-app)",
        borderColor: active ? "var(--accent)" : "var(--border-default)",
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

/**
 * Settings → Goals.
 *
 * Reference: `screens/SCR-20260514-rjjq-2.png`.
 *
 *   Daily hours goal                                                9h
 *   1h ────────●────────────────── 14h
 *
 *   How many hours you aim to work each working day. Used in the Goals view.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getPomodoroConfig, setPomodoroConfig } from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";
import { useT } from "../../i18n";
import { firstError, goalSliderHoursSchema } from "../../lib/validation";
import { usePrefsStore } from "../../stores/prefsStore";

import { NonWorkingDaysList } from "./NonWorkingDaysList";
import { SettingsCard } from "./SettingsCard";
import { WorkingWeekMask } from "./WorkingWeekMask";

const MIN_HOURS = 1;
const MAX_HOURS = 14;

export default function SettingsGoals() {
  const t = useT();
  const goalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const setGoal = usePrefsStore((s) => s.setDailyGoal);

  const hours = Math.max(MIN_HOURS, Math.min(MAX_HOURS, goalSeconds / 3600));

  return (
    <div className="flex flex-col gap-5 w-full max-w-3xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {t("settingsGoals.heading")}
        </h2>
      </header>

      <SettingsCard
        title={t("settingsGoals.dailyGoal.title")}
        description={t("settingsGoals.dailyGoal.description")}
        action={
          <span className="text-lg font-semibold text-[var(--accent)] tabular-nums">
            {hours}h
          </span>
        }
      >
        <input
          type="range"
          min={MIN_HOURS}
          max={MAX_HOURS}
          step={0.5}
          value={hours}
          onChange={(e) => {
            const h = parseFloat(e.target.value);
            // The slider already constrains to [1, 14] at 0.5 steps, but
            // re-run the schema as a belt-and-braces guard against bad
            // input from accessibility tools.
            if (firstError(goalSliderHoursSchema, h)) return;
            void setGoal(Math.round(h * 3600));
          }}
          className="w-full h-1.5 rounded-full appearance-none cursor-pointer
                     bg-[var(--accent-soft)] outline-none"
          style={{
            // Custom range thumb color via accent.
            // CSS variable in a string isn't easily themable via Tailwind here.
            accentColor: "var(--accent)",
          }}
        />
        <div className="flex items-center justify-between mt-1 text-[11px] text-[var(--text-tertiary)] tabular-nums">
          <span>{MIN_HOURS}h</span>
          <span>{MAX_HOURS}h</span>
        </div>
      </SettingsCard>

      <SettingsCard>
        <WorkingWeekMask />
      </SettingsCard>

      <SettingsCard>
        <NonWorkingDaysList />
      </SettingsCard>

      <SettingsCard>
        <PomodoroSection />
      </SettingsCard>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pomodoro — fokusovat 25/5 cykly (konfigurovatelné). Frontend pouze ukládá
// nastavení; samotná notifikace běží přes `usePomodoroTimer` v `App.tsx`.
// ---------------------------------------------------------------------------

function PomodoroSection() {
  const t = useT();
  const q = useQuery({
    queryKey: queryKeys.pomodoroConfig.all(),
    queryFn: getPomodoroConfig,
    staleTime: 60_000,
  });
  const queryClient = useQueryClient();
  const cfg = q.data ?? { enabled: false, work_min: 25, break_min: 5 };
  const [work, setWork] = useState(cfg.work_min);
  const [brk, setBrk] = useState(cfg.break_min);
  useEffect(() => {
    setWork(cfg.work_min);
    setBrk(cfg.break_min);
  }, [cfg.work_min, cfg.break_min]);

  const save = async (next: { enabled?: boolean; work_min?: number; break_min?: number }) => {
    const payload = {
      enabled: next.enabled ?? cfg.enabled,
      work_min: next.work_min ?? work,
      break_min: next.break_min ?? brk,
    };
    try {
      await setPomodoroConfig(payload);
      queryClient.invalidateQueries({ queryKey: queryKeys.pomodoroConfig.all() });
    } catch {
      /* swallow — UI validation pokrývá rozsah */
    }
  };

  return (
    <section className="flex flex-col gap-3">
      <h3 className="text-sm font-semibold text-[var(--text-primary)]">
        {t("settingsGoals.pomodoro.title")}
      </h3>
      <label className="flex items-start gap-2 cursor-pointer select-none">
        <input
          type="checkbox"
          checked={cfg.enabled}
          onChange={(e) => void save({ enabled: e.target.checked })}
          className="mt-0.5 accent-[var(--accent)]"
        />
        <span className="text-xs text-[var(--text-secondary)]">
          <span className="font-medium text-[var(--text-primary)]">
            {t("settingsGoals.pomodoro.enable")}
          </span>
          <br />
          {t("settingsGoals.pomodoro.description")}
        </span>
      </label>
      {cfg.enabled && (
        <div className="grid grid-cols-2 gap-3 max-w-xs pl-6">
          <NumberField
            id="pomo-work"
            label={t("settingsGoals.pomodoro.work")}
            value={work}
            min={5}
            max={180}
            onChange={(v) => {
              setWork(v);
              void save({ work_min: v });
            }}
          />
          <NumberField
            id="pomo-break"
            label={t("settingsGoals.pomodoro.break")}
            value={brk}
            min={1}
            max={60}
            onChange={(v) => {
              setBrk(v);
              void save({ break_min: v });
            }}
          />
        </div>
      )}
    </section>
  );
}

function NumberField({
  id,
  label,
  value,
  min,
  max,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  return (
    <label htmlFor={id} className="flex flex-col gap-1 text-xs">
      <span className="text-[var(--text-secondary)]">{label}</span>
      <input
        id={id}
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(e) => {
          const n = Number(e.target.value);
          if (Number.isFinite(n) && n >= min && n <= max) onChange(n);
        }}
        className="ui-input tabular-nums"
      />
    </label>
  );
}

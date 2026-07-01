/**
 * Settings → Appearance.
 *
 * Reference: `screens/SCR-20260514-rjks-2.png` (dark) and `rlbv-2.png`,
 * `rled-2.png` (light).
 *
 *   Theme    [☀ Light] [☾ Dark*] [□ System]
 *
 *   Color palette                                       [ Mono | Dual ]
 *   ┌──────────┐ ┌──────────┐ ┌──────────┐
 *   │   ▆▆▆▆▆  │ │   ▆▆▆▆▆  │ │   ▆▆▆▆▆  │
 *   │   ▇▇▇    │ │   ▇▇▇    │ │   ▇▇▇    │
 *   │   ▇▇▇▇▇▇ │ │   ▇▇▇▇▇▇ │ │   ▇▇▇▇▇▇ │
 *   │ ● Tyrkys │ │ ● Tracker│ │ ● Love   │
 *   └──────────┘ └──────────┘ └──────────┘
 *
 * Palette previews use 4 rows of accent-tinted bars: the top row uses the
 * primary, the rows below use lighter tints (mono) or secondary (dual).
 *
 * Note: the original reference shows lock icons next to the Dual palettes.
 * Per the brief, ALL palettes are unlocked in our app.
 */
import { Check, Monitor, Moon, Sun } from "lucide-react";

import type { PaletteMode, ThemePref } from "../../api/types";
import {
  DUAL_PALETTES,
  MONO_PALETTES,
  type PaletteSpec,
} from "../../lib/accent";
import { useT } from "../../i18n";
import { usePrefsStore } from "../../stores/prefsStore";
import { SettingsCard } from "./SettingsCard";

const THEME_OPTIONS: { value: ThemePref; labelKey: string; icon: React.ReactNode }[] = [
  { value: "light", labelKey: "settingsMisc.appearance.themeLight", icon: <Sun className="w-4 h-4" aria-hidden /> },
  { value: "dark", labelKey: "settingsMisc.appearance.themeDark", icon: <Moon className="w-4 h-4" aria-hidden /> },
  { value: "auto", labelKey: "settingsMisc.appearance.themeSystem", icon: <Monitor className="w-4 h-4" aria-hidden /> },
];

export default function Appearance() {
  const t = useT();
  const language = usePrefsStore((s) => s.language);
  const setLanguage = usePrefsStore((s) => s.setLanguage);
  const theme = usePrefsStore((s) => s.theme);
  const setTheme = usePrefsStore((s) => s.setTheme);
  const accent = usePrefsStore((s) => s.accent);
  const setAccent = usePrefsStore((s) => s.setAccent);
  const paletteMode = usePrefsStore((s) => s.paletteMode);
  const setPaletteMode = usePrefsStore((s) => s.setPaletteMode);
  const timelineStartHour = usePrefsStore((s) => s.timelineStartHour);
  const timelineEndHour = usePrefsStore((s) => s.timelineEndHour);
  const setTimelineHours = usePrefsStore((s) => s.setTimelineHours);

  const palettes = paletteMode === "dual" ? DUAL_PALETTES : MONO_PALETTES;

  return (
    <div className="flex flex-col gap-5 w-full max-w-3xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          {t("settingsMisc.appearance.title")}
        </h2>
      </header>

      <SettingsCard
        title={t("settings.language.title")}
        description={t("settings.language.description")}
      >
        <div
          className="inline-flex items-center rounded-full p-0.5 text-xs"
          style={{ background: "var(--bg-active)" }}
        >
          {(["cs", "en"] as const).map((code) => (
            <button
              key={code}
              type="button"
              onClick={() => void setLanguage(code)}
              className="px-3 h-7 rounded-full transition-colors duration-150"
              style={
                language === code
                  ? { background: "var(--accent-soft)", color: "var(--accent)" }
                  : { color: "var(--text-tertiary)" }
              }
            >
              {t(`settings.language.${code}`)}
            </button>
          ))}
        </div>
      </SettingsCard>

      <SettingsCard
        title={t("settingsMisc.appearance.themeTitle")}
        description={t("settingsMisc.appearance.themeDescription")}
      >
        <div className="grid grid-cols-3 gap-3">
          {THEME_OPTIONS.map((opt) => (
            <ThemeCard
              key={opt.value}
              label={t(opt.labelKey)}
              icon={opt.icon}
              active={theme === opt.value}
              onClick={() => void setTheme(opt.value)}
            />
          ))}
        </div>
      </SettingsCard>

      <SettingsCard
        title={t("settingsMisc.appearance.paletteTitle")}
        description={t("settingsMisc.appearance.paletteDescription")}
        action={
          <PaletteModeToggle
            value={paletteMode}
            onChange={(m) => void setPaletteMode(m)}
          />
        }
      >
        <div className="grid grid-cols-3 gap-3">
          {palettes.map((p) => (
            <PaletteCard
              key={p.id}
              palette={p}
              active={accent === p.id}
              onClick={() => void setAccent(p.id as never)}
            />
          ))}
        </div>
      </SettingsCard>

      <SettingsCard
        title={t("settingsMisc.appearance.timelineTitle")}
        description={t("settingsMisc.appearance.timelineDescription")}
      >
        <TimelineRangeControl
          startHour={timelineStartHour}
          endHour={timelineEndHour}
          onChange={(s, e) => void setTimelineHours(s, e)}
        />
      </SettingsCard>
    </div>
  );
}

/** Two-hours (0–24) day-timeline window picker with a full-day preset. */
function TimelineRangeControl({
  startHour,
  endHour,
  onChange,
}: {
  startHour: number;
  endHour: number;
  onChange: (startHour: number, endHour: number) => void;
}) {
  const t = useT();
  const isFullDay = startHour === 0 && endHour === 24;
  const hh = (h: number) => `${String(h).padStart(2, "0")}:00`;
  // Constrain the option lists so `start < end` always holds without silent
  // auto-bumps: `od` offers 0..end-1, `do` offers start+1..24.
  const startOptions = Array.from({ length: endHour }, (_, h) => h);
  const endOptions = Array.from({ length: 24 - startHour }, (_, i) => startHour + 1 + i);

  return (
    <div className="flex flex-col gap-3">
      <div
        className="inline-flex items-center rounded-full p-0.5 text-xs self-start"
        style={{ background: "var(--bg-active)" }}
      >
        {(
          [
            { full: true, label: t("settingsMisc.appearance.timelineFullDay") },
            { full: false, label: t("settingsMisc.appearance.timelineCustomRange") },
          ] as const
        ).map((opt) => {
          const active = opt.full === isFullDay;
          return (
            <button
              key={opt.label}
              type="button"
              onClick={() => {
                if (opt.full) onChange(0, 24);
                // Switching to custom from full-day seeds a sensible window.
                else if (isFullDay) onChange(6, 22);
              }}
              className="px-3 h-6 rounded-full transition-colors duration-150"
              style={
                active
                  ? { background: "var(--accent-soft)", color: "var(--accent)" }
                  : { color: "var(--text-tertiary)" }
              }
            >
              {opt.label}
            </button>
          );
        })}
      </div>

      {!isFullDay && (
        <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
          <label className="flex items-center gap-1.5">
            <span>{t("settingsMisc.appearance.timelineFrom")}</span>
            <select
              className="ui-select"
              value={startHour}
              onChange={(e) => onChange(Number(e.target.value), endHour)}
            >
              {startOptions.map((h) => (
                <option key={h} value={h}>
                  {hh(h)}
                </option>
              ))}
            </select>
          </label>
          <span className="text-[var(--text-tertiary)]">–</span>
          <label className="flex items-center gap-1.5">
            <span>{t("settingsMisc.appearance.timelineTo")}</span>
            <select
              className="ui-select"
              value={endHour}
              onChange={(e) => onChange(startHour, Number(e.target.value))}
            >
              {endOptions.map((h) => (
                <option key={h} value={h}>
                  {hh(h)}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
    </div>
  );
}

function ThemeCard({
  label,
  icon,
  active,
  onClick,
}: {
  label: string;
  icon: React.ReactNode;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex flex-col items-center justify-center gap-2 h-20
                 rounded-[var(--radius-lg)] border transition-colors duration-150"
      style={{
        background: active ? "var(--accent-soft)" : "var(--bg-app)",
        borderColor: active ? "var(--accent)" : "var(--border-default)",
        color: active ? "var(--accent)" : "var(--text-primary)",
      }}
    >
      <span style={{ color: active ? "var(--accent)" : "var(--text-secondary)" }}>
        {icon}
      </span>
      <span className="text-xs">{label}</span>
    </button>
  );
}

const PALETTE_MODE_LABEL_KEY: Record<PaletteMode, string> = {
  mono: "settingsMisc.appearance.paletteModeMono",
  dual: "settingsMisc.appearance.paletteModeDual",
};

function PaletteModeToggle({
  value,
  onChange,
}: {
  value: PaletteMode;
  onChange: (v: PaletteMode) => void;
}) {
  const t = useT();
  return (
    <div className="inline-flex items-center rounded-full p-0.5 text-xs"
         style={{ background: "var(--bg-active)" }}>
      {(["mono", "dual"] as PaletteMode[]).map((m) => (
        <button
          key={m}
          type="button"
          onClick={() => onChange(m)}
          className="px-3 h-6 rounded-full transition-colors duration-150"
          style={
            value === m
              ? {
                  background: "var(--accent-soft)",
                  color: "var(--accent)",
                }
              : { color: "var(--text-tertiary)" }
          }
        >
          {t(PALETTE_MODE_LABEL_KEY[m])}
        </button>
      ))}
    </div>
  );
}

function PaletteCard({
  palette,
  active,
  onClick,
}: {
  palette: PaletteSpec;
  active: boolean;
  onClick: () => void;
}) {
  const isDual = palette.mode === "dual";
  return (
    <button
      type="button"
      onClick={onClick}
      className="relative rounded-[var(--radius-lg)] p-3 text-left transition-colors duration-150
                 border"
      style={{
        background: "var(--bg-app)",
        borderColor: active ? palette.primary : "var(--border-default)",
        boxShadow: active ? `0 0 0 1px ${palette.primary}` : undefined,
      }}
    >
      <div className="rounded-[var(--radius-md)] p-3 flex flex-col gap-1.5"
           style={{ background: "var(--bg-surface)" }}>
        <PaletteBar color={palette.primary} width="55%" tone="solid" />
        <PaletteBar
          color={isDual ? palette.secondary : palette.primary}
          width="85%"
          tone="soft"
        />
        <PaletteBar color={palette.primary} width="40%" tone="soft" />
        <PaletteBar
          color={isDual ? palette.secondary : palette.primary}
          width="70%"
          tone="solid"
        />
      </div>
      <div className="mt-3 flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span
            aria-hidden
            className="w-2 h-2 rounded-full"
            style={{ background: palette.primary }}
          />
          <span className="text-xs text-[var(--text-primary)]">{palette.label}</span>
        </div>
        {active && (
          <Check
            className="w-3.5 h-3.5"
            style={{ color: palette.primary }}
            aria-hidden
          />
        )}
      </div>
    </button>
  );
}

function PaletteBar({
  color,
  width,
  tone,
}: {
  color: string;
  width: string;
  tone: "solid" | "soft";
}) {
  return (
    <div className="flex items-center gap-1.5">
      <div
        className="h-1.5 rounded-full"
        style={{
          background: tone === "solid" ? color : "var(--border-default)",
          width: "30%",
        }}
      />
      <div
        className="h-1.5 rounded-full"
        style={{
          background: tone === "solid" ? color : `${color}55`,
          width,
        }}
      />
    </div>
  );
}

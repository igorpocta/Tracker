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
 *   │ ● SAB    │ │ ● Tracker│ │ ● Love   │
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
import { usePrefsStore } from "../../stores/prefsStore";

const THEME_OPTIONS: { value: ThemePref; label: string; icon: React.ReactNode }[] = [
  { value: "light", label: "Světlý", icon: <Sun className="w-4 h-4" aria-hidden /> },
  { value: "dark", label: "Tmavý", icon: <Moon className="w-4 h-4" aria-hidden /> },
  { value: "auto", label: "Systémový", icon: <Monitor className="w-4 h-4" aria-hidden /> },
];

export default function Appearance() {
  const theme = usePrefsStore((s) => s.theme);
  const setTheme = usePrefsStore((s) => s.setTheme);
  const accent = usePrefsStore((s) => s.accent);
  const setAccent = usePrefsStore((s) => s.setAccent);
  const paletteMode = usePrefsStore((s) => s.paletteMode);
  const setPaletteMode = usePrefsStore((s) => s.setPaletteMode);

  const palettes = paletteMode === "dual" ? DUAL_PALETTES : MONO_PALETTES;

  return (
    <div className="flex flex-col gap-8 max-w-2xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Vzhled
        </h2>
      </header>

      {/* Theme picker --------------------------------------------------- */}
      <section>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">
          Motiv
        </h3>
        <div className="grid grid-cols-3 gap-3">
          {THEME_OPTIONS.map((opt) => (
            <ThemeCard
              key={opt.value}
              label={opt.label}
              icon={opt.icon}
              active={theme === opt.value}
              onClick={() => void setTheme(opt.value)}
            />
          ))}
        </div>
      </section>

      {/* Palettes ------------------------------------------------------- */}
      <section>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-semibold text-[var(--text-primary)]">
            Barevná paleta
          </h3>
          <PaletteModeToggle value={paletteMode} onChange={(m) => void setPaletteMode(m)} />
        </div>
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
      </section>
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
        background: active ? "var(--accent-soft)" : "var(--bg-surface)",
        borderColor: active ? "var(--accent)" : "var(--border-subtle)",
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

const PALETTE_MODE_LABEL: Record<PaletteMode, string> = {
  mono: "Mono",
  dual: "Duální",
};

function PaletteModeToggle({
  value,
  onChange,
}: {
  value: PaletteMode;
  onChange: (v: PaletteMode) => void;
}) {
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
          {PALETTE_MODE_LABEL[m]}
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
        background: "var(--bg-surface)",
        borderColor: active ? palette.primary : "var(--border-subtle)",
        boxShadow: active ? `0 0 0 1px ${palette.primary}` : undefined,
      }}
    >
      <div className="rounded-[var(--radius-md)] p-3 flex flex-col gap-1.5"
           style={{ background: "var(--bg-app)" }}>
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

/**
 * Settings → Appearance tab.
 *
 * Sections: accent color swatches, theme, font size, density, plus a live
 * worklog-row preview reflecting the current settings.
 */
import { RadioGroup } from "../../components/common/Radio";
import { Card } from "../../components/common/Card";
import { AccentSwatchRow } from "../../components/Settings/AccentSwatchRow";
import { formatClockTime, formatDurationShort } from "../../lib/format";
import {
  type DensityPref,
  type FontSizePref,
  type ThemePref,
} from "../../api/types";
import { usePrefsStore } from "../../stores/prefsStore";

const THEME_OPTIONS: { value: ThemePref; label: string; hint?: string }[] = [
  { value: "auto", label: "Auto", hint: "Follow system" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

const FONT_OPTIONS: { value: FontSizePref; label: string; hint?: string }[] = [
  { value: "sm", label: "Small", hint: "13 px" },
  { value: "md", label: "Medium", hint: "14 px" },
  { value: "lg", label: "Large", hint: "16 px" },
];

const DENSITY_OPTIONS: { value: DensityPref; label: string; hint?: string }[] = [
  { value: "compact", label: "Compact" },
  { value: "comfortable", label: "Comfortable" },
];

export default function Appearance() {
  const theme = usePrefsStore((s) => s.theme);
  const fontSize = usePrefsStore((s) => s.fontSize);
  const density = usePrefsStore((s) => s.density);
  const accent = usePrefsStore((s) => s.accent);
  const setTheme = usePrefsStore((s) => s.setTheme);
  const setFontSize = usePrefsStore((s) => s.setFontSize);
  const setDensity = usePrefsStore((s) => s.setDensity);
  const setAccent = usePrefsStore((s) => s.setAccent);

  const previewStart = Date.now() - 3600 * 1000;
  const previewEnd = Date.now();

  return (
    <div className="flex flex-col gap-7 max-w-xl">
      <Section
        title="Accent color"
        description="Used for the timer, primary actions, and selection."
      >
        <AccentSwatchRow value={accent} onChange={(v) => void setAccent(v)} />
      </Section>

      <Section
        title="Theme"
        description="Switch between light and dark — auto follows your OS."
      >
        <RadioGroup<ThemePref>
          label="Theme"
          options={THEME_OPTIONS}
          value={theme}
          onChange={(v) => void setTheme(v)}
        />
      </Section>

      <Section
        title="Font size"
        description="Scales the whole UI proportionally."
      >
        <RadioGroup<FontSizePref>
          label="Font size"
          options={FONT_OPTIONS}
          value={fontSize}
          onChange={(v) => void setFontSize(v)}
        />
      </Section>

      <Section
        title="Density"
        description="Compact removes padding from list rows."
      >
        <RadioGroup<DensityPref>
          label="Density"
          options={DENSITY_OPTIONS}
          value={density}
          onChange={(v) => void setDensity(v)}
        />
      </Section>

      <Section
        title="Preview"
        description="A worklog row rendered with your current settings."
      >
        <Card padding="none">
          <ul className="flex flex-col">
            <li className="worklog-row group rounded-[var(--radius-sm)] px-3 flex items-start gap-3">
              <div className="mt-2 shrink-0">
                <span
                  aria-hidden
                  className="block w-1.5 h-1.5 rounded-full bg-[var(--accent)]"
                />
              </div>
              <div className="shrink-0 font-mono tabular-nums text-[11px] text-[var(--text-tertiary)] w-[88px] mt-0.5">
                {formatClockTime(previewStart)}–{formatClockTime(previewEnd)}
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-[11px] uppercase text-[var(--text-secondary)] shrink-0">
                    ACME-123
                  </span>
                  <span className="text-xs text-[var(--text-primary)] truncate">
                    Tracker frontend redesign
                  </span>
                </div>
                <p className="text-xs text-[var(--text-tertiary)] mt-0.5 line-clamp-2">
                  Reworked the home view into Today / History / Reports tabs.
                </p>
              </div>
              <div className="text-right shrink-0">
                <div className="font-mono tabular-nums text-xs text-[var(--text-primary)]">
                  {formatDurationShort(3600)}
                </div>
              </div>
            </li>
          </ul>
        </Card>
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
    <section className="flex flex-col gap-3">
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">{title}</h3>
        {description && (
          <p className="text-xs text-[var(--text-tertiary)] mt-0.5">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

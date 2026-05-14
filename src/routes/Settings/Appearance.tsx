/**
 * Settings → Appearance tab.
 *
 * Three radio groups (theme, font size, density) that write through to the
 * prefs store. The store applies the values to the DOM immediately so any
 * change is visible.
 *
 * A small live "Preview" card shows how a worklog row will look with the
 * current settings.
 */
import { CheckCircle2 } from "lucide-react";

import { RadioGroup } from "../../components/common/Radio";
import { Card } from "../../components/common/Card";
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
  const setTheme = usePrefsStore((s) => s.setTheme);
  const setFontSize = usePrefsStore((s) => s.setFontSize);
  const setDensity = usePrefsStore((s) => s.setDensity);

  const previewStart = Date.now() - 3600 * 1000;
  const previewEnd = Date.now();

  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <Section title="Theme" description="Switch between light and dark — auto follows your OS.">
        <RadioGroup<ThemePref>
          label="Theme"
          options={THEME_OPTIONS}
          value={theme}
          onChange={(v) => void setTheme(v)}
        />
      </Section>

      <Section title="Font size" description="Scales the whole UI proportionally.">
        <RadioGroup<FontSizePref>
          label="Font size"
          options={FONT_OPTIONS}
          value={fontSize}
          onChange={(v) => void setFontSize(v)}
        />
      </Section>

      <Section title="Density" description="Compact removes padding from list rows.">
        <RadioGroup<DensityPref>
          label="Density"
          options={DENSITY_OPTIONS}
          value={density}
          onChange={(v) => void setDensity(v)}
        />
      </Section>

      <Section title="Preview" description="A worklog row rendered with your current settings.">
        <Card padding="none">
          <ul className="flex flex-col">
            <li className="worklog-row group rounded-md px-3 border border-transparent hover:bg-neutral-800/50 hover:border-neutral-800 flex items-start gap-3">
              <div className="mt-1 shrink-0">
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" aria-hidden />
              </div>
              <div className="shrink-0 font-mono tabular-nums text-[11px] text-neutral-400 w-[88px] mt-0.5">
                {formatClockTime(previewStart)}–{formatClockTime(previewEnd)}
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-[11px] text-neutral-400 shrink-0">ACME-123</span>
                  <span className="text-xs text-neutral-200 truncate">
                    Tracker frontend redesign
                  </span>
                </div>
                <p className="text-xs text-neutral-400 mt-0.5 line-clamp-2">
                  Reworked the home view into Today / History / Reports tabs.
                </p>
              </div>
              <div className="text-right shrink-0">
                <div className="font-mono text-xs text-neutral-100">
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
    <section className="flex flex-col gap-2">
      <div>
        <h3 className="text-sm font-semibold">{title}</h3>
        {description && (
          <p className="text-xs text-neutral-500 mt-0.5">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

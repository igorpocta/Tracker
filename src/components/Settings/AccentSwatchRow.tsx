/**
 * Row of accent-color swatches for Settings → Appearance.
 *
 * Each swatch is a rounded square showing the color's preview hex; the
 * selected swatch gets a thin ring + check overlay so it's unambiguous which
 * is active. Clicking writes through `setAccent` in the prefs store, which
 * persists to the backend and re-applies the CSS hue immediately.
 */
import { Check } from "lucide-react";

import type { AccentColor } from "../../api/types";
import { ACCENTS } from "../../lib/accent";

export interface AccentSwatchRowProps {
  value: AccentColor;
  onChange: (next: AccentColor) => void;
}

export function AccentSwatchRow({ value, onChange }: AccentSwatchRowProps) {
  return (
    <div
      role="radiogroup"
      aria-label="Accent color"
      className="inline-flex flex-wrap gap-2"
      data-testid="accent-swatches"
    >
      {ACCENTS.map((a) => {
        const selected = a.id === value;
        return (
          <button
            key={a.id}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={a.label}
            title={a.label}
            onClick={() => onChange(a.id)}
            data-accent={a.id}
            data-selected={selected ? "true" : "false"}
            className="relative w-7 h-7 rounded-[var(--radius-sm)] transition-transform duration-150 hover:scale-105 focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--text-tertiary)] ring-offset-2 ring-offset-[var(--bg-surface)]"
            style={{
              background: a.swatch,
              boxShadow: selected
                ? `0 0 0 2px var(--bg-surface), 0 0 0 4px ${a.swatch}`
                : undefined,
            }}
          >
            {selected && (
              <Check
                className="absolute inset-0 m-auto w-3.5 h-3.5 text-white drop-shadow"
                aria-hidden
              />
            )}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Accent color palette.
 *
 * Each entry maps a stable identifier (stored in `app_settings.accent_color`)
 * to a set of HSL components which are written to `<html>` as CSS custom
 * properties — `--accent-h`, `--accent-s`, `--accent-l`. The rest of the
 * design-token system derives `--accent`, `--accent-hover`, `--accent-soft`
 * etc. from those three numbers, so a single swatch click recolors the
 * entire UI consistently.
 *
 * "Graphite" is the neutral fallback: zero saturation so accent-tinted
 * surfaces collapse to gray, useful for users who want a flat monochrome UI.
 */
import type { AccentColor } from "../api/types";

export interface AccentSpec {
  id: AccentColor;
  /** User-facing label. */
  label: string;
  /** Hue (0-360). */
  h: number;
  /** Saturation percent (0-100). */
  s: number;
  /** Lightness percent (0-100). */
  l: number;
  /** Hex preview swatch color (purely for the picker UI). */
  swatch: string;
}

export const ACCENTS: AccentSpec[] = [
  { id: "blue",     label: "Blue",     h: 211, s: 100, l: 50, swatch: "#0a84ff" },
  { id: "indigo",   label: "Indigo",   h: 244, s: 90,  l: 60, swatch: "#5e5ce6" },
  { id: "violet",   label: "Violet",   h: 270, s: 80,  l: 60, swatch: "#bf5af2" },
  { id: "pink",     label: "Pink",     h: 340, s: 90,  l: 60, swatch: "#ff375f" },
  { id: "red",      label: "Red",      h: 4,   s: 90,  l: 56, swatch: "#ff453a" },
  { id: "orange",   label: "Orange",   h: 28,  s: 100, l: 52, swatch: "#ff9f0a" },
  { id: "yellow",   label: "Yellow",   h: 48,  s: 100, l: 50, swatch: "#ffd60a" },
  { id: "green",    label: "Green",    h: 142, s: 70,  l: 45, swatch: "#34c759" },
  { id: "teal",     label: "Teal",     h: 178, s: 75,  l: 42, swatch: "#30b0c7" },
  { id: "graphite", label: "Graphite", h: 220, s: 4,   l: 50, swatch: "#86868b" },
];

export const ACCENT_INDEX: Record<AccentColor, AccentSpec> = ACCENTS.reduce(
  (acc, spec) => {
    acc[spec.id] = spec;
    return acc;
  },
  {} as Record<AccentColor, AccentSpec>,
);

/** Returns the spec for an accent id, falling back to "blue" for unknown. */
export function getAccentSpec(id: string): AccentSpec {
  return ACCENT_INDEX[id as AccentColor] ?? ACCENT_INDEX.blue;
}

/**
 * Apply the accent to the document root by writing the three HSL components
 * as CSS custom properties. The tokens defined in `index.css` derive every
 * other accent-related value from these, so this single call recolors the
 * entire UI atomically.
 */
export function applyAccent(id: string): void {
  if (typeof document === "undefined") return;
  const spec = getAccentSpec(id);
  const root = document.documentElement.style;
  root.setProperty("--accent-h", `${spec.h}`);
  root.setProperty("--accent-s", `${spec.s}%`);
  root.setProperty("--accent-l", `${spec.l}%`);
}

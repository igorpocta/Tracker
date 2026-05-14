/**
 * Color palette system.
 *
 * The original Tracker reference exposes two palette modes:
 *
 *   1. "Mono" — a single primary accent (Aurora / Tracker / Love / Halloween).
 *   2. "Dual" — primary + secondary accents (Czech / Aurora Boreal /
 *      Sakura Night / Cyber Lime / Nordic Fjord) so charts and pills can
 *      adopt a "second voice" without ever feeling garish.
 *
 * Every palette resolves to four CSS custom properties on `<html>`:
 *
 *   --accent          (primary, hex)
 *   --accent-soft     (primary at 15% opacity)
 *   --accent-2        (secondary, hex — equals primary in mono)
 *   --accent-2-soft   (secondary at 15% opacity — equals primary-soft in mono)
 *
 * Components that previously read `--accent-hover` etc. derived from HSL
 * components still work because we also write `--accent-h/s/l` for the
 * primary color so the legacy derivations remain meaningful.
 */

export type PaletteMode = "mono" | "dual";

export interface PaletteSpec {
  /** Stable identifier, also stored in the DB. */
  id: string;
  /** User-facing label. */
  label: string;
  /** "mono" or "dual" — used to filter the palette previews. */
  mode: PaletteMode;
  /** Primary color hex. */
  primary: string;
  /** Secondary color hex. Equals `primary` for mono palettes. */
  secondary: string;
}

/** Mono palettes — single primary accent. */
export const MONO_PALETTES: PaletteSpec[] = [
  { id: "aurora",    label: "SAB",       mode: "mono", primary: "#14B8A6", secondary: "#14B8A6" },
  { id: "trcker",    label: "Tracker",   mode: "mono", primary: "#EAB308", secondary: "#EAB308" },
  { id: "love",      label: "Love",      mode: "mono", primary: "#EC4899", secondary: "#EC4899" },
  { id: "halloween", label: "Halloween", mode: "mono", primary: "#F97316", secondary: "#F97316" },
  // Phase 18B — Item 16: six new MONO palettes.
  { id: "graphite",  label: "Graphite",  mode: "mono", primary: "#71717A", secondary: "#71717A" },
  { id: "mocha",     label: "Mocha",     mode: "mono", primary: "#A87C5F", secondary: "#A87C5F" },
  { id: "electric",  label: "Electric",  mode: "mono", primary: "#3B82F6", secondary: "#3B82F6" },
  { id: "forest",    label: "Forest",    mode: "mono", primary: "#22C55E", secondary: "#22C55E" },
  { id: "plum",      label: "Plum",      mode: "mono", primary: "#A855F7", secondary: "#A855F7" },
  { id: "rust",      label: "Rust",      mode: "mono", primary: "#DC2626", secondary: "#DC2626" },
];

/** Dual palettes — primary + secondary, both UNLOCKED (no premium gate). */
export const DUAL_PALETTES: PaletteSpec[] = [
  { id: "czech",         label: "Czech",         mode: "dual", primary: "#3B82F6", secondary: "#DC2626" },
  { id: "aurora-boreal", label: "Aurora Boreal", mode: "dual", primary: "#14B8A6", secondary: "#22C55E" },
  { id: "sakura-night",  label: "Sakura Night",  mode: "dual", primary: "#EC4899", secondary: "#A855F7" },
  { id: "cyber-lime",    label: "Cyber Lime",    mode: "dual", primary: "#84CC16", secondary: "#7C3AED" },
  { id: "nordic-fjord",  label: "Nordic Fjord",  mode: "dual", primary: "#F59E0B", secondary: "#0EA5E9" },
];

export const ALL_PALETTES: PaletteSpec[] = [...MONO_PALETTES, ...DUAL_PALETTES];

export const PALETTE_INDEX: Record<string, PaletteSpec> = ALL_PALETTES.reduce(
  (acc, p) => {
    acc[p.id] = p;
    return acc;
  },
  {} as Record<string, PaletteSpec>,
);

/** Default palette id. */
export const DEFAULT_PALETTE_ID = "aurora";

/** Returns the spec for a palette id, falling back to the default. */
export function getPaletteSpec(id: string): PaletteSpec {
  return PALETTE_INDEX[id] ?? PALETTE_INDEX[DEFAULT_PALETTE_ID];
}

/** Returns true if `id` corresponds to a known dual palette. */
export function isDualPalette(id: string): boolean {
  return PALETTE_INDEX[id]?.mode === "dual";
}

// -----------------------------------------------------------------------------
// Hex → RGB / HSL helpers (for the CSS variables).
// -----------------------------------------------------------------------------

interface RGB {
  r: number;
  g: number;
  b: number;
}

function hexToRgb(hex: string): RGB {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  if (!m) return { r: 20, g: 184, b: 166 };
  return {
    r: parseInt(m[1], 16),
    g: parseInt(m[2], 16),
    b: parseInt(m[3], 16),
  };
}

function rgbToHsl({ r, g, b }: RGB): { h: number; s: number; l: number } {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case rn:
        h = ((gn - bn) / d + (gn < bn ? 6 : 0)) * 60;
        break;
      case gn:
        h = ((bn - rn) / d + 2) * 60;
        break;
      default:
        h = ((rn - gn) / d + 4) * 60;
    }
  }
  return { h, s: s * 100, l: l * 100 };
}

/** Build a `rgba()` string from a hex + 0..1 alpha. */
function rgba(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Apply a palette to the document root by writing CSS custom properties.
 *
 * Writes:
 *   --accent, --accent-hover, --accent-soft, --accent-strong, --accent-ring
 *   --accent-2, --accent-2-soft
 *   --accent-h / --accent-s / --accent-l  (legacy HSL components)
 */
export function applyPalette(id: string): void {
  if (typeof document === "undefined") return;
  const spec = getPaletteSpec(id);
  const root = document.documentElement.style;

  // Primary
  root.setProperty("--accent", spec.primary);
  root.setProperty("--accent-hover", lighten(spec.primary, 0.08));
  root.setProperty("--accent-soft", rgba(spec.primary, 0.15));
  root.setProperty("--accent-strong", rgba(spec.primary, 0.28));
  root.setProperty("--accent-ring", rgba(spec.primary, 0.35));

  // Secondary (= primary in mono mode)
  root.setProperty("--accent-2", spec.secondary);
  root.setProperty("--accent-2-soft", rgba(spec.secondary, 0.15));

  // Legacy HSL components: a few old utilities reference these. Keep them
  // in sync with the primary so nothing breaks visually.
  const { h, s, l } = rgbToHsl(hexToRgb(spec.primary));
  root.setProperty("--accent-h", `${h.toFixed(1)}`);
  root.setProperty("--accent-s", `${s.toFixed(1)}%`);
  root.setProperty("--accent-l", `${l.toFixed(1)}%`);
}

/** Backward-compatible alias used in popover bootstrapping. */
export const applyAccent = applyPalette;

/** Lighten a hex by `amount` (0..1) — used for the hover state. */
function lighten(hex: string, amount: number): string {
  const { r, g, b } = hexToRgb(hex);
  const adjust = (c: number) => Math.min(255, Math.round(c + (255 - c) * amount));
  return `rgb(${adjust(r)}, ${adjust(g)}, ${adjust(b)})`;
}

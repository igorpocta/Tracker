/**
 * Zustand store for user preferences.
 *
 * Mirrors the backend-backed prefs (daily goal, hourly rate, theme, font size,
 * density, accent color, currency, widget format).
 *
 * Setters write through the relevant Tauri command, then patch local state.
 * The backend emits `prefs-changed` for cross-window sync — subscribers wire
 * that up themselves and call `hydrate()` again to pick up fresh values.
 *
 * As a side effect, this module also applies theme / font-size / density /
 * accent color to the DOM (`<html data-theme>`, CSS variables, body class)
 * so each change is immediately visible to the user.
 */
import { create } from "zustand";

import {
  getAccentColor,
  getCurrency,
  getDailyGoal,
  getDayTimelineVisible,
  getDensity,
  getFontSize,
  getHourlyRate,
  getPaletteMode,
  getTheme,
  setAccentColor as invokeSetAccentColor,
  setAppIcon as invokeSetAppIcon,
  setCurrency as invokeSetCurrency,
  setDailyGoal as invokeSetDailyGoal,
  setDayTimelineVisible as invokeSetDayTimelineVisible,
  setDensity as invokeSetDensity,
  setFontSize as invokeSetFontSize,
  setHourlyRate as invokeSetHourlyRate,
  setPaletteMode as invokeSetPaletteMode,
  setTheme as invokeSetTheme,
  setWidgetFormat as invokeSetWidgetFormat,
} from "../api/commands";
import type {
  AccentColor,
  Currency,
  DensityPref,
  FontSizePref,
  PaletteMode,
  ThemePref,
} from "../api/types";
import { applyPalette, DEFAULT_PALETTE_ID, isDualPalette } from "../lib/accent";

export const DEFAULT_DAILY_GOAL_SECONDS = 8 * 60 * 60;
export const DEFAULT_HOURLY_RATE = 0;
export const DEFAULT_THEME: ThemePref = "auto";
export const DEFAULT_FONT_SIZE: FontSizePref = "md";
export const DEFAULT_DENSITY: DensityPref = "comfortable";
export const DEFAULT_ACCENT: AccentColor = DEFAULT_PALETTE_ID as AccentColor;
export const DEFAULT_PALETTE_MODE: PaletteMode = "mono";
export const DEFAULT_CURRENCY: Currency = "CZK";
export const DEFAULT_DAY_TIMELINE_VISIBLE = true;

/** Widget time display format. */
export type WidgetFormat = "HH:MM:SS" | "Hh Mm" | "0.0h";
export const DEFAULT_WIDGET_FORMAT: WidgetFormat = "HH:MM:SS";

const LS_WIDGET_FORMAT_KEY = "tracker.widgetFormat";

export interface PrefsStoreState {
  /** Daily goal in seconds. Defaults to 8 hours when unset. */
  dailyGoalSeconds: number;
  /** Hourly rate in user's currency. 0 means "not set / hide the row". */
  hourlyRate: number;
  /** Currency code for hourly-rate display. Backend-backed. */
  currency: Currency;
  /** Widget time format. */
  widgetFormat: WidgetFormat;
  /** Theme preference (`auto`/`light`/`dark`). */
  theme: ThemePref;
  /** Font-size preference (`sm`/`md`/`lg`). */
  fontSize: FontSizePref;
  /** Density preference (`compact`/`comfortable`). */
  density: DensityPref;
  /** Accent palette identifier. */
  accent: AccentColor;
  /** Mono vs Dual palette mode. */
  paletteMode: PaletteMode;
  /** Whether to render the DayTimeline on the Time Log route. */
  dayTimelineVisible: boolean;
  /** True until the first hydrate completes — used to avoid flicker. */
  hydrated: boolean;
  error: string | null;
}

export interface PrefsStoreActions {
  hydrate: () => Promise<void>;
  setDailyGoal: (seconds: number) => Promise<void>;
  setHourlyRate: (rate: number) => Promise<void>;
  setCurrency: (currency: Currency) => Promise<void>;
  setWidgetFormat: (format: WidgetFormat) => Promise<void>;
  setTheme: (theme: ThemePref) => Promise<void>;
  setFontSize: (size: FontSizePref) => Promise<void>;
  setDensity: (density: DensityPref) => Promise<void>;
  setAccent: (accent: AccentColor) => Promise<void>;
  setPaletteMode: (mode: PaletteMode) => Promise<void>;
  setDayTimelineVisible: (visible: boolean) => Promise<void>;
  setAppIcon: (icon: string) => Promise<void>;
}

export type PrefsStore = PrefsStoreState & PrefsStoreActions;

// ---- DOM application helpers -------------------------------------------------

/**
 * Apply the theme preference to the document root. With `auto` we just clear
 * the `data-theme` attribute so the `prefers-color-scheme` media query takes
 * over; explicit values pin the scheme regardless of system setting.
 */
export function applyTheme(theme: ThemePref): void {
  if (typeof document === "undefined") return;
  const html = document.documentElement;
  if (theme === "auto") {
    html.removeAttribute("data-theme");
  } else {
    html.setAttribute("data-theme", theme);
  }
}

/** Map font-size preference to a base pixel size. */
export function fontSizeToPx(size: FontSizePref): number {
  switch (size) {
    case "sm":
      return 13;
    case "lg":
      return 16;
    case "md":
    default:
      return 14;
  }
}

/** Set the CSS variable that scales typography across the app. */
export function applyFontSize(size: FontSizePref): void {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty(
    "--base-font-size",
    `${fontSizeToPx(size)}px`,
  );
}

/** Toggle the body density class. */
export function applyDensity(density: DensityPref): void {
  if (typeof document === "undefined") return;
  const body = document.body;
  body.classList.remove("density-compact", "density-comfortable");
  body.classList.add(
    density === "compact" ? "density-compact" : "density-comfortable",
  );
}

function readWidgetFormat(): WidgetFormat {
  if (typeof window === "undefined") return DEFAULT_WIDGET_FORMAT;
  try {
    const raw = window.localStorage.getItem(LS_WIDGET_FORMAT_KEY);
    if (raw === "HH:MM:SS" || raw === "Hh Mm" || raw === "0.0h") return raw;
  } catch {
    /* ignore */
  }
  return DEFAULT_WIDGET_FORMAT;
}

function writeWidgetFormat(f: WidgetFormat): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(LS_WIDGET_FORMAT_KEY, f);
  } catch {
    /* ignore */
  }
}

const KNOWN_ACCENTS: ReadonlySet<string> = new Set<string>([
  // Legacy hues
  "blue",
  "indigo",
  "violet",
  "pink",
  "red",
  "orange",
  "yellow",
  "green",
  "teal",
  "graphite",
  // Mono palettes
  "aurora",
  "trcker",
  "love",
  "halloween",
  // Phase 18B — Item 16: new MONO palettes
  "mocha",
  "electric",
  "forest",
  "plum",
  "rust",
  // Dual palettes
  "czech",
  "aurora-boreal",
  "sakura-night",
  "cyber-lime",
  "nordic-fjord",
]);

function isAccentId(v: string): v is AccentColor {
  return KNOWN_ACCENTS.has(v);
}

function isPaletteMode(v: string): v is PaletteMode {
  return v === "mono" || v === "dual";
}

function isCurrencyCode(v: string): v is Currency {
  return (
    v === "CZK" ||
    v === "EUR" ||
    v === "USD" ||
    v === "GBP" ||
    v === "PLN" ||
    v === "CHF"
  );
}

export const usePrefsStore = create<PrefsStore>((set) => ({
  dailyGoalSeconds: DEFAULT_DAILY_GOAL_SECONDS,
  hourlyRate: DEFAULT_HOURLY_RATE,
  currency: DEFAULT_CURRENCY,
  widgetFormat: readWidgetFormat(),
  theme: DEFAULT_THEME,
  fontSize: DEFAULT_FONT_SIZE,
  density: DEFAULT_DENSITY,
  accent: DEFAULT_ACCENT,
  paletteMode: DEFAULT_PALETTE_MODE,
  dayTimelineVisible: DEFAULT_DAY_TIMELINE_VISIBLE,
  hydrated: false,
  error: null,

  hydrate: async () => {
    try {
      const [
        goal,
        rate,
        theme,
        fontSize,
        density,
        accentRaw,
        currencyRaw,
        paletteModeRaw,
        dayTimelineVisible,
      ] = await Promise.all([
        getDailyGoal(),
        getHourlyRate(),
        getTheme().catch(() => DEFAULT_THEME),
        getFontSize().catch(() => DEFAULT_FONT_SIZE),
        getDensity().catch(() => DEFAULT_DENSITY),
        getAccentColor().catch(() => DEFAULT_ACCENT as string),
        getCurrency().catch(() => DEFAULT_CURRENCY as string),
        getPaletteMode().catch(() => DEFAULT_PALETTE_MODE as string),
        getDayTimelineVisible().catch(() => DEFAULT_DAY_TIMELINE_VISIBLE),
      ]);
      const accent: AccentColor = isAccentId(accentRaw)
        ? accentRaw
        : DEFAULT_ACCENT;
      const currency: Currency = isCurrencyCode(currencyRaw)
        ? currencyRaw
        : DEFAULT_CURRENCY;
      // Auto-derive the mode from the accent id when possible — keeps the UI
      // consistent with the actual palette even if the stored `palette_mode`
      // drifts (e.g. user picked a dual palette from a list).
      const derivedMode: PaletteMode = isDualPalette(accent) ? "dual" : "mono";
      const paletteMode: PaletteMode = isPaletteMode(paletteModeRaw)
        ? (paletteModeRaw as PaletteMode)
        : derivedMode;

      applyTheme(theme);
      applyFontSize(fontSize);
      applyDensity(density);
      applyPalette(accent);
      set({
        dailyGoalSeconds: goal,
        hourlyRate: rate,
        theme,
        fontSize,
        density,
        accent,
        currency,
        paletteMode,
        dayTimelineVisible,
        hydrated: true,
        error: null,
      });
    } catch (e) {
      set({ hydrated: true, error: errMessage(e) });
    }
  },

  setDailyGoal: async (seconds) => {
    await invokeSetDailyGoal(seconds);
    set({ dailyGoalSeconds: seconds });
  },

  setHourlyRate: async (rate) => {
    await invokeSetHourlyRate(rate);
    set({ hourlyRate: rate });
  },

  setCurrency: async (currency) => {
    await invokeSetCurrency(currency);
    set({ currency });
  },

  setWidgetFormat: async (format) => {
    writeWidgetFormat(format);
    set({ widgetFormat: format });
    // Pass-through to the backend so the tray widget picks it up.
    try {
      await invokeSetWidgetFormat(format);
    } catch {
      /* swallow — backend wiring is best-effort outside Tauri. */
    }
  },

  setTheme: async (theme) => {
    await invokeSetTheme(theme);
    applyTheme(theme);
    set({ theme });
  },

  setFontSize: async (size) => {
    await invokeSetFontSize(size);
    applyFontSize(size);
    set({ fontSize: size });
  },

  setDensity: async (density) => {
    await invokeSetDensity(density);
    applyDensity(density);
    set({ density });
  },

  setAccent: async (accent) => {
    await invokeSetAccentColor(accent);
    applyPalette(accent);
    // Po změně palety přebarvit APP ikonu (dock na macOS, taskbar/window
    // jinde) podle vybrané palety. Tichá best-effort akce — failuje-li
    // (např. nepodporovaná platforma), aplikace běží dál s původní ikonou.
    try {
      const spec = (await import("../lib/accent")).getPaletteSpec(accent);
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("set_app_icon_accent", {
        primary: spec.primary,
        secondary: spec.secondary !== spec.primary ? spec.secondary : null,
      });
    } catch {
      /* tauri API není dostupné (web preview, test) */
    }
    // Keep the palette mode in sync with the accent's natural mode so the
    // Mono/Dual toggle UI always reflects reality.
    const derivedMode: PaletteMode = isDualPalette(accent) ? "dual" : "mono";
    set({ accent, paletteMode: derivedMode });
    try {
      await invokeSetPaletteMode(derivedMode);
    } catch {
      /* backend wiring optional in tests / older builds */
    }
  },

  setPaletteMode: async (mode) => {
    try {
      await invokeSetPaletteMode(mode);
    } catch {
      /* swallow — frontend can still drive the picker UI */
    }
    set({ paletteMode: mode });
  },

  setDayTimelineVisible: async (visible) => {
    try {
      await invokeSetDayTimelineVisible(visible);
    } catch {
      /* swallow — best-effort; the UI state is still authoritative */
    }
    set({ dayTimelineVisible: visible });
  },

  setAppIcon: async (icon) => {
    await invokeSetAppIcon(icon);
  },
}));

function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "unknown error";
}

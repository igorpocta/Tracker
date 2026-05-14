/**
 * Zustand store for user preferences.
 *
 * Mirrors the backend-backed prefs (daily goal, hourly rate, theme, font size,
 * density) plus a couple of locally-persisted ones (currency, widget format)
 * that don't have a dedicated backend command yet.
 *
 * Setters write through the relevant Tauri command, then patch local state.
 * The backend emits `prefs-changed` for cross-window sync — subscribers wire
 * that up themselves and call `hydrate()` again to pick up fresh values.
 *
 * As a side effect, this module also applies theme / font-size / density to
 * the DOM (`<html data-theme>`, CSS variable, body class) so the change is
 * immediately visible to the user.
 */
import { create } from "zustand";

import {
  getDailyGoal,
  getDensity,
  getFontSize,
  getHourlyRate,
  getTheme,
  setAppIcon as invokeSetAppIcon,
  setDailyGoal as invokeSetDailyGoal,
  setDensity as invokeSetDensity,
  setFontSize as invokeSetFontSize,
  setHourlyRate as invokeSetHourlyRate,
  setTheme as invokeSetTheme,
  setWidgetFormat as invokeSetWidgetFormat,
} from "../api/commands";
import type { DensityPref, FontSizePref, ThemePref } from "../api/types";

export const DEFAULT_DAILY_GOAL_SECONDS = 8 * 60 * 60;
export const DEFAULT_HOURLY_RATE = 0;
export const DEFAULT_THEME: ThemePref = "auto";
export const DEFAULT_FONT_SIZE: FontSizePref = "md";
export const DEFAULT_DENSITY: DensityPref = "comfortable";

/** Supported currency codes for the hourly-rate display unit. */
export type Currency = "CZK" | "EUR" | "USD";
export const DEFAULT_CURRENCY: Currency = "CZK";

/** Widget time display format. */
export type WidgetFormat = "HH:MM:SS" | "Hh Mm" | "0.0h";
export const DEFAULT_WIDGET_FORMAT: WidgetFormat = "HH:MM:SS";

const LS_CURRENCY_KEY = "tracker.currency";
const LS_WIDGET_FORMAT_KEY = "tracker.widgetFormat";

export interface PrefsStoreState {
  /** Daily goal in seconds. Defaults to 8 hours when unset. */
  dailyGoalSeconds: number;
  /** Hourly rate in user's currency. 0 means "not set / hide the row". */
  hourlyRate: number;
  /** Currency code for hourly-rate display. */
  currency: Currency;
  /** Widget time format. */
  widgetFormat: WidgetFormat;
  /** Theme preference (`auto`/`light`/`dark`). */
  theme: ThemePref;
  /** Font-size preference (`sm`/`md`/`lg`). */
  fontSize: FontSizePref;
  /** Density preference (`compact`/`comfortable`). */
  density: DensityPref;
  /** True until the first hydrate completes — used to avoid flicker. */
  hydrated: boolean;
  error: string | null;
}

export interface PrefsStoreActions {
  hydrate: () => Promise<void>;
  setDailyGoal: (seconds: number) => Promise<void>;
  setHourlyRate: (rate: number) => Promise<void>;
  setCurrency: (currency: Currency) => void;
  setWidgetFormat: (format: WidgetFormat) => Promise<void>;
  setTheme: (theme: ThemePref) => Promise<void>;
  setFontSize: (size: FontSizePref) => Promise<void>;
  setDensity: (density: DensityPref) => Promise<void>;
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

function readCurrency(): Currency {
  if (typeof window === "undefined") return DEFAULT_CURRENCY;
  try {
    const raw = window.localStorage.getItem(LS_CURRENCY_KEY);
    if (raw === "CZK" || raw === "EUR" || raw === "USD") return raw;
  } catch {
    /* ignore */
  }
  return DEFAULT_CURRENCY;
}

function writeCurrency(c: Currency): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(LS_CURRENCY_KEY, c);
  } catch {
    /* ignore */
  }
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

export const usePrefsStore = create<PrefsStore>((set) => ({
  dailyGoalSeconds: DEFAULT_DAILY_GOAL_SECONDS,
  hourlyRate: DEFAULT_HOURLY_RATE,
  currency: readCurrency(),
  widgetFormat: readWidgetFormat(),
  theme: DEFAULT_THEME,
  fontSize: DEFAULT_FONT_SIZE,
  density: DEFAULT_DENSITY,
  hydrated: false,
  error: null,

  hydrate: async () => {
    try {
      const [goal, rate, theme, fontSize, density] = await Promise.all([
        getDailyGoal(),
        getHourlyRate(),
        getTheme().catch(() => DEFAULT_THEME),
        getFontSize().catch(() => DEFAULT_FONT_SIZE),
        getDensity().catch(() => DEFAULT_DENSITY),
      ]);
      applyTheme(theme);
      applyFontSize(fontSize);
      applyDensity(density);
      set({
        dailyGoalSeconds: goal,
        hourlyRate: rate,
        theme,
        fontSize,
        density,
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

  setCurrency: (currency) => {
    writeCurrency(currency);
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

  setAppIcon: async (icon) => {
    await invokeSetAppIcon(icon);
  },
}));

function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "unknown error";
}

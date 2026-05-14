/**
 * Zustand store for user preferences (daily goal, hourly rate, …).
 *
 * Setters write through the relevant Tauri command, then patch local state.
 * The backend emits `prefs-changed` for cross-window sync — subscribers wire
 * that up themselves and call `hydrate()` again to pick up fresh values.
 */
import { create } from "zustand";

import {
  getDailyGoal,
  getHourlyRate,
  setAppIcon as invokeSetAppIcon,
  setDailyGoal as invokeSetDailyGoal,
  setHourlyRate as invokeSetHourlyRate,
  setWidgetFormat as invokeSetWidgetFormat,
} from "../api/commands";

export const DEFAULT_DAILY_GOAL_SECONDS = 8 * 60 * 60;
export const DEFAULT_HOURLY_RATE = 0;

export interface PrefsStoreState {
  /** Daily goal in seconds. Defaults to 8 hours when unset. */
  dailyGoalSeconds: number;
  /** Hourly rate in user's currency. 0 means "not set / hide the row". */
  hourlyRate: number;
  /** True until the first hydrate completes — used to avoid flicker. */
  hydrated: boolean;
  error: string | null;
}

export interface PrefsStoreActions {
  hydrate: () => Promise<void>;
  setDailyGoal: (seconds: number) => Promise<void>;
  setHourlyRate: (rate: number) => Promise<void>;
  setWidgetFormat: (format: string) => Promise<void>;
  setAppIcon: (icon: string) => Promise<void>;
}

export type PrefsStore = PrefsStoreState & PrefsStoreActions;

export const usePrefsStore = create<PrefsStore>((set) => ({
  dailyGoalSeconds: DEFAULT_DAILY_GOAL_SECONDS,
  hourlyRate: DEFAULT_HOURLY_RATE,
  hydrated: false,
  error: null,

  hydrate: async () => {
    try {
      const [goal, rate] = await Promise.all([
        getDailyGoal(),
        getHourlyRate(),
      ]);
      set({
        dailyGoalSeconds: goal,
        hourlyRate: rate,
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

  setWidgetFormat: async (format) => {
    await invokeSetWidgetFormat(format);
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

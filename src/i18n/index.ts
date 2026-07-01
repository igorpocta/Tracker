/**
 * React binding for the i18n catalogue. `useT()` returns a `t(key, vars?)`
 * function bound to the current UI language from the prefs store, so components
 * re-render when the user switches language.
 */
import { useMemo } from "react";

import { usePrefsStore } from "../stores/prefsStore";

import { MESSAGES, translate, type Language, type TFunc } from "./messages";

export { MESSAGES, translate };
export type { Language, TFunc };

export function useT(): TFunc {
  const lang = usePrefsStore((s) => s.language);
  return useMemo<TFunc>(() => (key, vars) => translate(lang, key, vars), [lang]);
}

/**
 * UI string catalogue (CZ default, EN secondary).
 *
 * Keys are dot-namespaced by feature (`nav.*`, `settings.*`, …). `translate`
 * resolves against the active language, falls back to Czech, and finally to the
 * key itself so a missing translation is visible rather than blank. `{var}`
 * placeholders are interpolated.
 *
 * This file is intentionally plain data + one pure function — no React, no
 * store — so it can be unit-tested and imported anywhere. The React binding
 * (`useT`) lives in `./index`.
 */

export type Language = "cs" | "en";

export type TFunc = (
  key: string,
  vars?: Record<string, string | number>,
) => string;

type Catalogue = Record<string, string>;

const cs: Catalogue = {
  // Navigation (IconSidebar)
  "nav.mainNav": "Hlavní navigace",
  "nav.home": "Tracker — domů",
  "nav.timeLog": "Časový záznam",
  "nav.unassigned": "Nepřiřazené",
  "nav.reports": "Reporty",
  "nav.calendar": "Kalendář",
  "nav.goals": "Cíle",
  "nav.audit": "Historie změn",
  "nav.jiraDashboard": "JIRA Přehled",
  "nav.settings": "Nastavení",
  "nav.unassignedBadge": "{count} nepřiřazených",

  // Settings → language switcher
  "settings.language.title": "Jazyk",
  "settings.language.description": "Jazyk uživatelského rozhraní.",
  "settings.language.cs": "Čeština",
  "settings.language.en": "Angličtina",
};

const en: Catalogue = {
  "nav.mainNav": "Main navigation",
  "nav.home": "Tracker — home",
  "nav.timeLog": "Time log",
  "nav.unassigned": "Unassigned",
  "nav.reports": "Reports",
  "nav.calendar": "Calendar",
  "nav.goals": "Goals",
  "nav.audit": "Change history",
  "nav.jiraDashboard": "Jira overview",
  "nav.settings": "Settings",
  "nav.unassignedBadge": "{count} unassigned",

  "settings.language.title": "Language",
  "settings.language.description": "User interface language.",
  "settings.language.cs": "Czech",
  "settings.language.en": "English",
};

export const MESSAGES: Record<Language, Catalogue> = { cs, en };

export function translate(
  lang: Language,
  key: string,
  vars?: Record<string, string | number>,
): string {
  const dict = MESSAGES[lang] ?? MESSAGES.cs;
  let s = dict[key] ?? MESSAGES.cs[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

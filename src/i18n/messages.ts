/**
 * UI string catalogue (CZ default, EN secondary).
 *
 * Strings live in per-feature namespace modules under `./messages/` (`nav`,
 * `settings`, …) so they can be migrated file-by-file without merge conflicts.
 * This module merges them into flat `cs` / `en` lookup tables and exposes the
 * pure `translate` helper. `translate` resolves against the active language,
 * falls back to Czech, and finally to the key itself so a missing translation
 * is visible rather than blank. `{var}` placeholders are interpolated.
 *
 * Adding a namespace: create `./messages/<ns>.ts` exporting
 * `export const <ns> = { cs: {...}, en: {...} } as const;`, then import it here
 * and spread it into `cs` / `en` below.
 */
import { connections } from "./messages/connections";
import { nav } from "./messages/nav";
import { settings } from "./messages/settings";
import { settingsGeneral } from "./messages/settingsGeneral";
import { settingsGoals } from "./messages/settingsGoals";
import { settingsMisc } from "./messages/settingsMisc";

export type Language = "cs" | "en";

export type TFunc = (
  key: string,
  vars?: Record<string, string | number>,
) => string;

type Catalogue = Record<string, string>;

const NAMESPACES = [
  nav,
  settings,
  settingsGeneral,
  settingsMisc,
  connections,
  settingsGoals,
];

const cs: Catalogue = Object.assign({}, ...NAMESPACES.map((n) => n.cs));
const en: Catalogue = Object.assign({}, ...NAMESPACES.map((n) => n.en));

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

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
import { audit } from "./messages/audit";
import { common } from "./messages/common";
import { connections } from "./messages/connections";
import { layout } from "./messages/layout";
import { misc } from "./messages/misc";
import { nav } from "./messages/nav";
import { reports } from "./messages/reports";
import { routes } from "./messages/routes";
import { settings } from "./messages/settings";
import { settingsGeneral } from "./messages/settingsGeneral";
import { settingsGoals } from "./messages/settingsGoals";
import { settingsMisc } from "./messages/settingsMisc";
import { setup } from "./messages/setup";
import { timeLog } from "./messages/timeLog";
import { timer } from "./messages/timer";
import { validation } from "./messages/validation";
import { worklog } from "./messages/worklog";

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
  timeLog,
  reports,
  routes,
  layout,
  timer,
  setup,
  audit,
  worklog,
  misc,
  common,
  validation,
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

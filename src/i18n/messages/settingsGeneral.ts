/** Settings → Obecné (General) strings. */
export const settingsGeneral = {
  cs: {
    "settingsGeneral.heading": "Obecné",

    "settingsGeneral.dayTimeline.title": "Časová osa dne",
    "settingsGeneral.dayTimeline.description":
      "Zobrazit nebo skrýt vizuální časovou osu nad záznamy.",
    "settingsGeneral.dayTimeline.visible": "Viditelná",
    "settingsGeneral.dayTimeline.hidden": "Skrytá",

    "settingsGeneral.timeInput.title": "Styl zadávání času",
    "settingsGeneral.timeInput.description":
      "Při přidávání záznamu zvolte, jestli preferujete nastavit koncový čas nebo trvání.",
    "settingsGeneral.timeInput.end": "Koncový čas — vyberte, kdy práce skončila",
    "settingsGeneral.timeInput.duration": "Trvání — zadejte počet minut",

    "settingsGeneral.rounding.title": "Zaokrouhlování času",
    "settingsGeneral.rounding.description":
      "Před uložením do Jiry můžete dobu zaokrouhlit nahoru nebo dolů na zvolený interval.",
    "settingsGeneral.rounding.none": "Žádné — uložit přesnou dobu",
    "settingsGeneral.rounding.up": "Nahoru — zaokrouhlit na další interval",
    "settingsGeneral.rounding.down": "Dolů — zaokrouhlit na předchozí interval",
    "settingsGeneral.rounding.intervalLabel": "Interval:",
    "settingsGeneral.rounding.interval1": "1 minuta",
    "settingsGeneral.rounding.interval5": "5 minut",
    "settingsGeneral.rounding.interval15": "15 minut",
    "settingsGeneral.rounding.interval60": "1 hodina",
    "settingsGeneral.rounding.saveModeError":
      "Nepodařilo se uložit režim zaokrouhlení.",
    "settingsGeneral.rounding.saveIntervalError":
      "Nepodařilo se uložit interval zaokrouhlení.",

    "settingsGeneral.shortcut.title": "Globální klávesová zkratka",
    "settingsGeneral.shortcut.description":
      "Systémová zkratka pro spuštění / zastavení časovače odkudkoli — funguje i mimo Tracker a když je okno skryté.",

    "settingsGeneral.autostart.title": "Spustit při přihlášení",
    "settingsGeneral.autostart.description":
      "Tracker se automaticky spustí při přihlášení do systému. Okno zůstane skryté — bude dostupné z menu baru.",
    "settingsGeneral.autostart.toggle": "Spouštět Tracker automaticky",

    "settingsGeneral.smartSuggestions.title": "Chytré návrhy úkolů",
    "settingsGeneral.smartSuggestions.description":
      'Banner "Jako včera?" navrhuje úkol, na kterém jste v podobný čas trackovali v posledních 14 dnech. Když ho vypnete, žádné návrhy se nezobrazují a backend se na ně ani neptá.',
    "settingsGeneral.smartSuggestions.toggle": "Zobrazovat chytré návrhy",

    "settingsGeneral.sentry.title": "Anonymní reportování chyb",
    "settingsGeneral.sentry.description":
      "Pokud zapnete, aplikace zasílá anonymizovaná hlášení chyb na Sentry — pomáhá nám diagnostikovat pády. API tokeny, hesla ani plné e-maily se neposílají. Identifikace je pouze anonymním instalačním ID.",
    "settingsGeneral.sentry.toggle": "Povolit reportování chyb",
    "settingsGeneral.sentry.note":
      "Změna se na frontendu projeví ihned. Backend přejde do nového režimu při příštím spuštění aplikace.",

    "settingsGeneral.activity.title": "Sledování aktivity",
    "settingsGeneral.activity.description":
      "Tracker sleduje, kdy s aplikací aktivně pracujete, a tuto informaci zobrazuje v přehledu cílů. Nemá vliv na uložené worklogy.",
    "settingsGeneral.activity.thresholdLabel": "Práh nečinnosti (minuty)",
    "settingsGeneral.activity.saveError": "Nepodařilo se uložit práh nečinnosti.",

    "settingsGeneral.reindex.title": "Interval automatické re-indexace",
    "settingsGeneral.reindex.description":
      "Jak často se na pozadí automaticky reindexují úkoly z Jiry. Interval se počítá od konce předchozí synchronizace — ne od fixní hodinové značky. Při startu aplikace proběhne první sync ihned (pokud nebyl proveden v posledních 60 minutách v debug buildu).",
    "settingsGeneral.reindex.manual": "Pouze ručně",
    "settingsGeneral.reindex.15m": "Každých 15 minut",
    "settingsGeneral.reindex.1h": "Každou hodinu",
    "settingsGeneral.reindex.4h": "Každé 4 hodiny",
    "settingsGeneral.reindex.daily": "Jednou denně",
    "settingsGeneral.reindex.notePrefix":
      "Reindexovat můžete také kdykoli ručně kliknutím na ikonu v liště nebo stisknutím ",

    "settingsGeneral.audit.title": "Historie změn",
    "settingsGeneral.audit.description":
      "Každá akce s worklogem (vytvoření, úprava, smazání, přesun) se ukládá do lokální historie. Z historie lze obnovit smazaný záznam zpět do Jiry nebo vrátit nedávnou úpravu.",
    "settingsGeneral.audit.open": "Otevřít historii změn",
    "settingsGeneral.audit.purgeLabel": "Vyprázdnit starší než",
    "settingsGeneral.audit.days": "dní",
    "settingsGeneral.audit.purgeButton": "Vyčistit",
    "settingsGeneral.audit.purgeConfirm": "Vyčistit",
    "settingsGeneral.audit.purgeDone.one": "Smazáno {n} záznam.",
    "settingsGeneral.audit.purgeDone.few": "Smazáno {n} záznamy.",
    "settingsGeneral.audit.purgeDone.many": "Smazáno {n} záznamů.",
    "settingsGeneral.audit.purgeFailed": "Vyčištění selhalo",
    "settingsGeneral.audit.purgeHint":
      "Smaže audit záznamy starší než zvolený počet dní. Tuto akci nelze vrátit.",

    "settingsGeneral.backup.title": "Záloha a obnova",
    "settingsGeneral.backup.description":
      "Export všech lokálních dat (worklogy, úkoly, nastavení) do JSON. Tokeny se neukládají — po obnově je nutné znovu zadat. Import přepíše stávající data.",
    "settingsGeneral.backup.download": "Stáhnout zálohu (.json)",
    "settingsGeneral.backup.restore": "Obnovit ze souboru…",
    "settingsGeneral.backup.exportDone": "Export hotov.",
    "settingsGeneral.backup.exportFailed": "Export selhal: {error}",
    "settingsGeneral.backup.importConfirm":
      'Importovat „{name}"?\n\nPOZOR: existující data v aplikaci budou přepsána. Pokračovat?',
    "settingsGeneral.backup.importDone":
      "Importováno: {worklogs} worklog(s), {issues} úkol(s), {connections} připojení.",
    "settingsGeneral.backup.importFailed": "Import selhal: {error}",
  },
  en: {
    "settingsGeneral.heading": "General",

    "settingsGeneral.dayTimeline.title": "Day timeline",
    "settingsGeneral.dayTimeline.description":
      "Show or hide the visual day timeline above the entries.",
    "settingsGeneral.dayTimeline.visible": "Visible",
    "settingsGeneral.dayTimeline.hidden": "Hidden",

    "settingsGeneral.timeInput.title": "Time input style",
    "settingsGeneral.timeInput.description":
      "When adding an entry, choose whether you prefer to set the end time or the duration.",
    "settingsGeneral.timeInput.end": "End time — pick when the work finished",
    "settingsGeneral.timeInput.duration": "Duration — enter the number of minutes",

    "settingsGeneral.rounding.title": "Time rounding",
    "settingsGeneral.rounding.description":
      "Before saving to Jira you can round the duration up or down to a chosen interval.",
    "settingsGeneral.rounding.none": "None — save the exact duration",
    "settingsGeneral.rounding.up": "Up — round to the next interval",
    "settingsGeneral.rounding.down": "Down — round to the previous interval",
    "settingsGeneral.rounding.intervalLabel": "Interval:",
    "settingsGeneral.rounding.interval1": "1 minute",
    "settingsGeneral.rounding.interval5": "5 minutes",
    "settingsGeneral.rounding.interval15": "15 minutes",
    "settingsGeneral.rounding.interval60": "1 hour",
    "settingsGeneral.rounding.saveModeError":
      "Could not save the rounding mode.",
    "settingsGeneral.rounding.saveIntervalError":
      "Could not save the rounding interval.",

    "settingsGeneral.shortcut.title": "Global keyboard shortcut",
    "settingsGeneral.shortcut.description":
      "A system-wide shortcut to start / stop the timer from anywhere — it works outside Tracker and even when the window is hidden.",

    "settingsGeneral.autostart.title": "Launch at login",
    "settingsGeneral.autostart.description":
      "Tracker starts automatically when you log in to the system. The window stays hidden — it will be available from the menu bar.",
    "settingsGeneral.autostart.toggle": "Launch Tracker automatically",

    "settingsGeneral.smartSuggestions.title": "Smart issue suggestions",
    "settingsGeneral.smartSuggestions.description":
      'The "Same as yesterday?" banner suggests an issue you tracked at a similar time in the last 14 days. When you turn it off, no suggestions are shown and the backend does not even ask for them.',
    "settingsGeneral.smartSuggestions.toggle": "Show smart suggestions",

    "settingsGeneral.sentry.title": "Anonymous error reporting",
    "settingsGeneral.sentry.description":
      "If you turn this on, the app sends anonymized error reports to Sentry — it helps us diagnose crashes. API tokens, passwords and full e-mails are not sent. Identification is only by an anonymous installation ID.",
    "settingsGeneral.sentry.toggle": "Enable error reporting",
    "settingsGeneral.sentry.note":
      "The change takes effect on the frontend immediately. The backend switches to the new mode the next time the app starts.",

    "settingsGeneral.activity.title": "Activity tracking",
    "settingsGeneral.activity.description":
      "Tracker monitors when you actively work with the app and shows this information in the goals overview. It does not affect saved worklogs.",
    "settingsGeneral.activity.thresholdLabel": "Inactivity threshold (minutes)",
    "settingsGeneral.activity.saveError": "Could not save the inactivity threshold.",

    "settingsGeneral.reindex.title": "Automatic reindex interval",
    "settingsGeneral.reindex.description":
      "How often issues from Jira are automatically reindexed in the background. The interval is counted from the end of the previous sync — not from a fixed hourly mark. On app start the first sync runs immediately (unless it was performed in the last 60 minutes in a debug build).",
    "settingsGeneral.reindex.manual": "Manually only",
    "settingsGeneral.reindex.15m": "Every 15 minutes",
    "settingsGeneral.reindex.1h": "Every hour",
    "settingsGeneral.reindex.4h": "Every 4 hours",
    "settingsGeneral.reindex.daily": "Once a day",
    "settingsGeneral.reindex.notePrefix":
      "You can also reindex manually at any time by clicking the icon in the toolbar or pressing ",

    "settingsGeneral.audit.title": "Change history",
    "settingsGeneral.audit.description":
      "Every worklog action (create, edit, delete, move) is saved to a local history. From the history you can restore a deleted entry back to Jira or undo a recent edit.",
    "settingsGeneral.audit.open": "Open change history",
    "settingsGeneral.audit.purgeLabel": "Purge older than",
    "settingsGeneral.audit.days": "days",
    "settingsGeneral.audit.purgeButton": "Clean up",
    "settingsGeneral.audit.purgeConfirm": "Clean up",
    "settingsGeneral.audit.purgeDone.one": "Deleted {n} record.",
    "settingsGeneral.audit.purgeDone.few": "Deleted {n} records.",
    "settingsGeneral.audit.purgeDone.many": "Deleted {n} records.",
    "settingsGeneral.audit.purgeFailed": "Clean up failed",
    "settingsGeneral.audit.purgeHint":
      "Deletes audit records older than the chosen number of days. This action cannot be undone.",

    "settingsGeneral.backup.title": "Backup and restore",
    "settingsGeneral.backup.description":
      "Export all local data (worklogs, issues, settings) to JSON. Tokens are not saved — after restore they must be entered again. Import overwrites existing data.",
    "settingsGeneral.backup.download": "Download backup (.json)",
    "settingsGeneral.backup.restore": "Restore from file…",
    "settingsGeneral.backup.exportDone": "Export done.",
    "settingsGeneral.backup.exportFailed": "Export failed: {error}",
    "settingsGeneral.backup.importConfirm":
      'Import "{name}"?\n\nWARNING: existing data in the app will be overwritten. Continue?',
    "settingsGeneral.backup.importDone":
      "Imported: {worklogs} worklog(s), {issues} issue(s), {connections} connection(s).",
    "settingsGeneral.backup.importFailed": "Import failed: {error}",
  },
} as const;

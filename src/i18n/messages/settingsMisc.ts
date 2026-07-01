/** Settings → O aplikaci / Reporting / Vzhled (remaining) strings. */
export const settingsMisc = {
  cs: {
    // About
    "settingsMisc.about.title": "O aplikaci",
    "settingsMisc.about.intro":
      "Tracker je lokální desktopový time-tracker pro Jira Cloud a Freelo. Spouští časomíry, zaznamenává worklogy do lokální SQLite databáze a synchronizuje je do připojených systémů. Všechna data zůstávají u vás na počítači — bez cloudu, bez účtu, bez telemetrie (krom volitelného anonymního reportu chyb).",
    "settingsMisc.about.loadError":
      "Nepodařilo se načíst informace o aplikaci: {error}",
    "settingsMisc.about.name": "Název",
    "settingsMisc.about.version": "Verze",
    "settingsMisc.about.tauri": "Tauri",
    "settingsMisc.about.openGithubTitle": "Otevřít repozitář na GitHubu",
    "settingsMisc.about.githubRepo": "GitHub repozitář",
    "settingsMisc.about.checking": "Kontroluji…",
    "settingsMisc.about.checkUpdates": "Zkontrolovat aktualizace",
    "settingsMisc.about.updateAvailable":
      "K dispozici je verze {version}. Nabídka k instalaci je v horní liště.",
    "settingsMisc.about.checkFailed": "Kontrola selhala: {error}",
    "settingsMisc.about.upToDate": "Máte nejnovější verzi.",

    // Reporting
    "settingsMisc.reporting.title": "Reporting",
    "settingsMisc.reporting.intro":
      "Nastavte hodinovou sazbu a uvidíte celkový výdělek v sekci Reporty. Výdělek zůstává skrytý za kliknutím na ikonu oka — vhodné při práci v open space.",
    "settingsMisc.reporting.hourlyRate": "Hodinová sazba",
    "settingsMisc.reporting.hourlyRateDescription":
      "Ponechte prázdné a karta výdělků se úplně skryje.",
    "settingsMisc.reporting.rateInvalid": "musí být platné číslo (např. 1500)",
    "settingsMisc.reporting.currency": "Měna",

    // Appearance (remaining)
    "settingsMisc.appearance.title": "Vzhled",
    "settingsMisc.appearance.themeTitle": "Motiv",
    "settingsMisc.appearance.themeDescription":
      "Světlý, tmavý, nebo podle nastavení systému.",
    "settingsMisc.appearance.themeLight": "Světlý",
    "settingsMisc.appearance.themeDark": "Tmavý",
    "settingsMisc.appearance.themeSystem": "Systémový",
    "settingsMisc.appearance.paletteTitle": "Barevná paleta",
    "settingsMisc.appearance.paletteDescription": "Barva zvýraznění v celé aplikaci.",
    "settingsMisc.appearance.paletteModeMono": "Mono",
    "settingsMisc.appearance.paletteModeDual": "Duální",
    "settingsMisc.appearance.timelineTitle": "Časová osa dne",
    "settingsMisc.appearance.timelineDescription":
      "Rozsah hodin na časové ose nad záznamy. Celý den ukáže 0–24, vlastní rozsah se hodí, když pracujete jen v části dne.",
    "settingsMisc.appearance.timelineFullDay": "Celý den (0–24)",
    "settingsMisc.appearance.timelineCustomRange": "Vlastní rozsah",
    "settingsMisc.appearance.timelineFrom": "Od",
    "settingsMisc.appearance.timelineTo": "Do",
  },
  en: {
    // About
    "settingsMisc.about.title": "About",
    "settingsMisc.about.intro":
      "Tracker is a local desktop time-tracker for Jira Cloud and Freelo. It runs timers, records worklogs into a local SQLite database and syncs them to connected systems. All data stays on your computer — no cloud, no account, no telemetry (aside from optional anonymous error reporting).",
    "settingsMisc.about.loadError": "Failed to load application info: {error}",
    "settingsMisc.about.name": "Name",
    "settingsMisc.about.version": "Version",
    "settingsMisc.about.tauri": "Tauri",
    "settingsMisc.about.openGithubTitle": "Open the repository on GitHub",
    "settingsMisc.about.githubRepo": "GitHub repository",
    "settingsMisc.about.checking": "Checking…",
    "settingsMisc.about.checkUpdates": "Check for updates",
    "settingsMisc.about.updateAvailable":
      "Version {version} is available. The install prompt is in the top bar.",
    "settingsMisc.about.checkFailed": "Check failed: {error}",
    "settingsMisc.about.upToDate": "You have the latest version.",

    // Reporting
    "settingsMisc.reporting.title": "Reporting",
    "settingsMisc.reporting.intro":
      "Set an hourly rate to see total earnings in the Reports section. Earnings stay hidden behind a click on the eye icon — handy when working in an open space.",
    "settingsMisc.reporting.hourlyRate": "Hourly rate",
    "settingsMisc.reporting.hourlyRateDescription":
      "Leave empty to hide the earnings card entirely.",
    "settingsMisc.reporting.rateInvalid": "must be a valid number (e.g. 1500)",
    "settingsMisc.reporting.currency": "Currency",

    // Appearance (remaining)
    "settingsMisc.appearance.title": "Appearance",
    "settingsMisc.appearance.themeTitle": "Theme",
    "settingsMisc.appearance.themeDescription": "Light, dark, or match the system setting.",
    "settingsMisc.appearance.themeLight": "Light",
    "settingsMisc.appearance.themeDark": "Dark",
    "settingsMisc.appearance.themeSystem": "System",
    "settingsMisc.appearance.paletteTitle": "Color palette",
    "settingsMisc.appearance.paletteDescription": "Accent color across the whole app.",
    "settingsMisc.appearance.paletteModeMono": "Mono",
    "settingsMisc.appearance.paletteModeDual": "Dual",
    "settingsMisc.appearance.timelineTitle": "Day timeline",
    "settingsMisc.appearance.timelineDescription":
      "Hour range on the timeline above the entries. Full day shows 0–24; a custom range is handy when you only work part of the day.",
    "settingsMisc.appearance.timelineFullDay": "Full day (0–24)",
    "settingsMisc.appearance.timelineCustomRange": "Custom range",
    "settingsMisc.appearance.timelineFrom": "From",
    "settingsMisc.appearance.timelineTo": "To",
  },
} as const;

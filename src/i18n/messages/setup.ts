/** Setup wizard + menu-bar popover strings. */
export const setup = {
  cs: {
    // Wizard shell + titles (Setup.tsx / Wizard.tsx)
    "setup.title.connectAccount": "Připojit účet",
    "setup.title.connectJira": "Připojit Jira",
    "setup.title.connectFreelo": "Připojit Freelo",
    "setup.wizard.regionLabel": "Průvodce nastavením",
    "setup.wizard.progressLabel": "Postup nastavení",
    "setup.wizard.stepCounter": "Krok {current} z {total}",
    "setup.error.missingConnectionId": "Vnitřní chyba: chybí id připojení",

    // Wizard step labels
    "setup.step.provider": "Poskytovatel",
    "setup.step.url": "URL",
    "setup.step.email": "E-mail",
    "setup.step.token": "Token",
    "setup.step.credentials": "Přihlášení",
    "setup.step.projects": "Projekty",

    // Provider picker (StepProvider.tsx)
    "setup.provider.jira.description": "Atlassian Jira (REST API v3, API token)",
    "setup.provider.freelo.description": "Freelo (REST API v1, e-mail + API klíč)",
    "setup.provider.toggl.description": "Toggl Track",
    "setup.provider.clockify.description": "Clockify",
    "setup.provider.choose": "Vyberte poskytovatele",
    "setup.provider.hint":
      "Připojení k jednomu nebo více poskytovatelům lze přidat později v Nastavení.",
    "setup.provider.soon": "Brzy",

    // Shared nav buttons
    "setup.button.next": "Další",
    "setup.button.back": "Zpět",
    "setup.button.finish": "Dokončit",
    "setup.button.testConnection": "Otestovat připojení",
    "setup.status.connectedAs": "Připojeno jako {name}",

    // Jira URL step (StepUrl.tsx)
    "setup.url.label": "Základní URL Jiry",
    "setup.url.hintPrefix": "Vaše Atlassian Cloud URL, např.",
    "setup.url.hintSuffix": ".",

    // Jira email step (StepEmail.tsx)
    "setup.email.label": "E-mail Atlassian účtu",
    "setup.email.hint": "E-mail propojený s vaším Atlassian účtem.",

    // Jira token step (StepToken.tsx)
    "setup.token.label": "Jira API token",
    "setup.token.placeholder": "vložte svůj token",
    "setup.token.hintPrefix": "Vytvořte si ho na",

    // Freelo credentials step (StepFreeloCreds.tsx)
    "setup.freelo.emailLabel": "Freelo e-mail",
    "setup.freelo.apiKeyLabel": "Freelo API klíč",
    "setup.freelo.apiKeyPlaceholder": "vložte svůj API klíč",
    "setup.freelo.apiKeyHintPrefix": "Najdete ho na",
    "setup.freelo.apiKeyHintPath": "Freelo → Profil → API klíč",
    "setup.freelo.advancedShow": "Zobrazit pokročilá nastavení",
    "setup.freelo.advancedHide": "Skrýt pokročilá nastavení",
    "setup.freelo.baseUrlLabel": "Freelo API URL",

    // Freelo projects step (StepFreeloProjects.tsx)
    "setup.projects.label": "Vyberte projekty",
    "setup.projects.hint":
      "Pouze úkoly z vybraných projektů se stáhnou a budou dostupné v tickeru.",
    "setup.projects.loading": "Načítám projekty…",
    "setup.projects.loadError": "Načtení projektů se nezdařilo",
    "setup.projects.empty": "Tento účet zatím nemá žádné projekty.",
    "setup.projects.searchPlaceholder": "Hledat projekt…",
    "setup.projects.selectedCount": "Vybráno {selected} z {total}.",

    // Menu-bar popover (popover.tsx)
    "setup.popover.dailyGoal": "Dnešní cíl",
    "setup.popover.noTimer": "Žádná časomíra neběží",
    "setup.popover.clickToStart": "Klikni na úkol pro spuštění",
    "setup.popover.recent": "Naposledy",
    "setup.popover.noRecent": "Zatím žádné nedávné úkoly.",
    "setup.popover.loadingIssue": "(načítá se…)",
    "setup.popover.openApp": "Otevřít aplikaci",
    "setup.popover.settings": "Nastavení",
    "setup.popover.quit": "Ukončit",
  },
  en: {
    // Wizard shell + titles (Setup.tsx / Wizard.tsx)
    "setup.title.connectAccount": "Connect account",
    "setup.title.connectJira": "Connect Jira",
    "setup.title.connectFreelo": "Connect Freelo",
    "setup.wizard.regionLabel": "Setup wizard",
    "setup.wizard.progressLabel": "Setup progress",
    "setup.wizard.stepCounter": "Step {current} of {total}",
    "setup.error.missingConnectionId": "Internal error: missing connection id",

    // Wizard step labels
    "setup.step.provider": "Provider",
    "setup.step.url": "URL",
    "setup.step.email": "Email",
    "setup.step.token": "Token",
    "setup.step.credentials": "Sign in",
    "setup.step.projects": "Projects",

    // Provider picker (StepProvider.tsx)
    "setup.provider.jira.description": "Atlassian Jira (REST API v3, API token)",
    "setup.provider.freelo.description": "Freelo (REST API v1, email + API key)",
    "setup.provider.toggl.description": "Toggl Track",
    "setup.provider.clockify.description": "Clockify",
    "setup.provider.choose": "Choose a provider",
    "setup.provider.hint":
      "You can add connections to one or more providers later in Settings.",
    "setup.provider.soon": "Soon",

    // Shared nav buttons
    "setup.button.next": "Next",
    "setup.button.back": "Back",
    "setup.button.finish": "Finish",
    "setup.button.testConnection": "Test connection",
    "setup.status.connectedAs": "Connected as {name}",

    // Jira URL step (StepUrl.tsx)
    "setup.url.label": "Jira base URL",
    "setup.url.hintPrefix": "Your Atlassian Cloud URL, e.g.",
    "setup.url.hintSuffix": ".",

    // Jira email step (StepEmail.tsx)
    "setup.email.label": "Atlassian account email",
    "setup.email.hint": "The email linked to your Atlassian account.",

    // Jira token step (StepToken.tsx)
    "setup.token.label": "Jira API token",
    "setup.token.placeholder": "paste your token",
    "setup.token.hintPrefix": "Create one at",

    // Freelo credentials step (StepFreeloCreds.tsx)
    "setup.freelo.emailLabel": "Freelo email",
    "setup.freelo.apiKeyLabel": "Freelo API key",
    "setup.freelo.apiKeyPlaceholder": "paste your API key",
    "setup.freelo.apiKeyHintPrefix": "Find it at",
    "setup.freelo.apiKeyHintPath": "Freelo → Profile → API key",
    "setup.freelo.advancedShow": "Show advanced settings",
    "setup.freelo.advancedHide": "Hide advanced settings",
    "setup.freelo.baseUrlLabel": "Freelo API URL",

    // Freelo projects step (StepFreeloProjects.tsx)
    "setup.projects.label": "Choose projects",
    "setup.projects.hint":
      "Only issues from the selected projects are downloaded and available in the tracker.",
    "setup.projects.loading": "Loading projects…",
    "setup.projects.loadError": "Failed to load projects",
    "setup.projects.empty": "This account has no projects yet.",
    "setup.projects.searchPlaceholder": "Search project…",
    "setup.projects.selectedCount": "Selected {selected} of {total}.",

    // Menu-bar popover (popover.tsx)
    "setup.popover.dailyGoal": "Today's goal",
    "setup.popover.noTimer": "No timer running",
    "setup.popover.clickToStart": "Click an issue to start",
    "setup.popover.recent": "Recent",
    "setup.popover.noRecent": "No recent issues yet.",
    "setup.popover.loadingIssue": "(loading…)",
    "setup.popover.openApp": "Open app",
    "setup.popover.settings": "Settings",
    "setup.popover.quit": "Quit",
  },
} as const;

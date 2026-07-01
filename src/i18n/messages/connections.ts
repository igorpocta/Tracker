/** Settings → Připojení + add/edit connection dialogs. */
export const connections = {
  cs: {
    // Connection.tsx — header + list
    "connections.title": "Připojení",
    "connections.subtitle":
      "Připojte jeden nebo více účtů. Můžete je pojmenovat a kdykoli upravit.",
    "connections.loading": "Načítám…",
    "connections.addNew": "Přidat nové připojení",
    "connections.empty":
      "Žádná připojení nejsou nakonfigurována. Klikněte na „Přidat nové připojení“ pro start.",

    // Connection card — full sync confirm
    "connections.fullSyncConfirm":
      "Stáhnout celou historii pro „{name}\"?\n\nToto stáhne všechny úkoly a worklogy ~10 let zpět a může chvíli trvat. Pro běžnou aktualizaci stačí tlačítko v levé liště.",

    // Connection card — test
    "connections.missingToken": "Chybí uložený token",
    "connections.testFailed": "Test se nezdařil",

    // Connection card — misc
    "connections.clickToRename": "Klikněte pro přejmenování",
    "connections.neverSynced": "nikdy nesyncováno",

    // Connection card — action buttons
    "connections.rename": "Přejmenovat",
    "connections.editCredentials": "Upravit přihlašovací údaje",
    "connections.edit": "Upravit",
    "connections.testConnection": "Otestovat připojení",
    "connections.test": "Test",
    "connections.fullSyncTooltip":
      "Stáhnout celou historii (úkoly + worklogy ~10 let)",
    "connections.disconnectConfirm": "Odpojit „{name}\"?",
    "connections.disconnect": "Odpojit",

    // Inline rename
    "connections.newName": "Nový název",

    // Freelo projects panel
    "connections.projectsLoadFailed": "Načtení projektů se nezdařilo",
    "connections.projectsLoading": "Načítám projekty…",
    "connections.noProjects": "Žádné projekty nenalezeny.",
    "connections.selectedCount": "Vybráno {selected} z {total}",
    "connections.hideProjects": "Skrýt projekty",
    "connections.selectedProjects": "Vybrané projekty",

    // Sync error labels
    "connections.syncError.connection": "Připojení",
    "connections.syncError.issues": "Načtení úkolů",
    "connections.syncError.worklogs": "Načtení záznamů",
    "connections.syncError.worklogsSkipped": "Worklog sync byl přeskočen",
    "connections.syncError.failedSuffix": "selhala",

    // Sync runs history
    "connections.hideSyncHistory": "Skrýt historii synchronizací",
    "connections.syncHistory": "Historie synchronizací",
    "connections.syncTable.when": "Kdy",
    "connections.syncTable.connection": "Připojení",
    "connections.syncTable.mode": "Režim",
    "connections.syncTable.issues": "Úkoly",
    "connections.syncTable.worklogs": "Worklogy",
    "connections.syncTable.duration": "Trvání",
    "connections.syncTable.status": "Stav",
    "connections.syncTable.noRecords": "Žádné záznamy.",
    "connections.syncMode.full": "celá historie",
    "connections.syncMode.incremental": "přírůstky",

    // Pagination
    "connections.pagination.label": "Stránkování",
    "connections.pagination.prev": "← Předchozí",
    "connections.pagination.next": "Další →",

    // Shared dialog fields / buttons
    "connections.save": "Uložit",
    "connections.cancel": "Zrušit",
    "connections.close": "Zavřít",
    "connections.testAction": "Otestovat",
    "connections.continue": "Pokračovat",
    "connections.back": "Zpět",
    "connections.connectedAs": "Připojeno jako {name}",
    "connections.connectionFailed": "Připojení se nezdařilo",
    "connections.saveFailed": "Uložení se nezdařilo",

    // Field labels
    "connections.field.name": "Název připojení",
    "connections.field.jiraUrl": "Základní URL Jiry",
    "connections.field.atlassianEmail": "E-mail Atlassian účtu",
    "connections.field.jiraToken": "Jira API token",
    "connections.field.freeloEmail": "Freelo e-mail",
    "connections.field.freeloApiKey": "Freelo API klíč",
    "connections.field.freeloApiUrl": "Freelo API URL",

    // Field placeholders
    "connections.placeholder.name": "např. SAB, Klient X, …",
    "connections.placeholder.nameShort": "např. SAB, Klient X",
    "connections.placeholder.token": "vložte svůj token",
    "connections.placeholder.apiKey": "vložte svůj API klíč",

    // Custom / self-hosted server checkbox
    "connections.customHost.title": "Vlastní / self-hosted server",
    "connections.customHost.descriptionPrefix": "Zapněte pro on-premise Jiru mimo ",
    "connections.customHost.descriptionSuffix":
      ". Ověřte, že URL je důvěryhodná — token se odešle na tento server.",

    // Advanced settings toggle
    "connections.hideAdvanced": "Skrýt pokročilá nastavení",
    "connections.showAdvanced": "Zobrazit pokročilá nastavení",

    // AddConnectionDialog — headers
    "connections.add.dialogLabel": "Přidat nové připojení",
    "connections.add.chooseProvider": "Vyberte poskytovatele",
    "connections.add.signIn": "přihlášení",
    "connections.add.chooseProjects": "Vyberte projekty",

    // EditConnectionDialog
    "connections.edit.dialogLabel": "Upravit {name}",
    "connections.edit.title": "Upravit {provider} připojení",
    "connections.edit.replaceToken": "Nahradit API token",
    "connections.edit.replaceApiKey": "Nahradit API klíč",
    "connections.edit.newToken": "Nový Jira API token",
    "connections.edit.newApiKey": "Nový Freelo API klíč",
    "connections.edit.statusesLoadFailed": "Statusy se nepodařilo načíst",

    // EditConnectionDialog — dashboard
    "connections.dashboard.title": "Zobrazit Dashboard",
    "connections.dashboard.description":
      "Přidá tuto Jiru do přehledové tabulky „JIRA Přehled\" v menu. Vyžaduje JQL filter níže.",
    "connections.dashboard.jqlLabel": "JQL pro Dashboard",
    "connections.dashboard.jqlHint":
      "Atlassian odmítne JQL bez aspoň jedné restrikce. Bez ORDER BY bere defaultní řazení dle Jiry.",

    // EditConnectionDialog — auto transition
    "connections.autoTransition.summary": "Automatický přechod stavu (volitelné)",
    "connections.autoTransition.fromLabel": "Pokud je úkol ve stavu…",
    "connections.autoTransition.toLabel": "…přejít při spuštění na",
    "connections.autoTransition.hint":
      "Tichá best-effort akce — pokud mezi vybranými stavy v Jiře neexistuje přímá transition, Tracker se ji prostě nepokusí provést (zapíše do logu). Necháte-li vybráno „—\", nic se nedělá.",

    // EditConnectionDialog — color
    "connections.color.title": "Vlastní barva v Reportech",
    "connections.color.description":
      "Když je vypnuto, použije se výchozí barva providera.",
    "connections.color.pickerLabel": "Barva pro toto připojení",

    // StatusSelect
    "connections.status.none": "— nezvoleno —",
    "connections.status.loading": "Načítám…",
  },
  en: {
    // Connection.tsx — header + list
    "connections.title": "Connections",
    "connections.subtitle":
      "Connect one or more accounts. You can name them and edit them anytime.",
    "connections.loading": "Loading…",
    "connections.addNew": "Add new connection",
    "connections.empty":
      "No connections are configured. Click “Add new connection” to get started.",

    // Connection card — full sync confirm
    "connections.fullSyncConfirm":
      "Download the entire history for “{name}”?\n\nThis will download all issues and worklogs going ~10 years back and may take a while. For a regular update, the button in the left bar is enough.",

    // Connection card — test
    "connections.missingToken": "Missing saved token",
    "connections.testFailed": "Test failed",

    // Connection card — misc
    "connections.clickToRename": "Click to rename",
    "connections.neverSynced": "never synced",

    // Connection card — action buttons
    "connections.rename": "Rename",
    "connections.editCredentials": "Edit credentials",
    "connections.edit": "Edit",
    "connections.testConnection": "Test connection",
    "connections.test": "Test",
    "connections.fullSyncTooltip":
      "Download entire history (issues + worklogs ~10 years)",
    "connections.disconnectConfirm": "Disconnect “{name}”?",
    "connections.disconnect": "Disconnect",

    // Inline rename
    "connections.newName": "New name",

    // Freelo projects panel
    "connections.projectsLoadFailed": "Failed to load projects",
    "connections.projectsLoading": "Loading projects…",
    "connections.noProjects": "No projects found.",
    "connections.selectedCount": "Selected {selected} of {total}",
    "connections.hideProjects": "Hide projects",
    "connections.selectedProjects": "Choose projects",

    // Sync error labels
    "connections.syncError.connection": "Connection",
    "connections.syncError.issues": "Loading issues",
    "connections.syncError.worklogs": "Loading records",
    "connections.syncError.worklogsSkipped": "Worklog sync was skipped",
    "connections.syncError.failedSuffix": "failed",

    // Sync runs history
    "connections.hideSyncHistory": "Hide sync history",
    "connections.syncHistory": "Sync history",
    "connections.syncTable.when": "When",
    "connections.syncTable.connection": "Connection",
    "connections.syncTable.mode": "Mode",
    "connections.syncTable.issues": "Issues",
    "connections.syncTable.worklogs": "Worklogs",
    "connections.syncTable.duration": "Duration",
    "connections.syncTable.status": "Status",
    "connections.syncTable.noRecords": "No records.",
    "connections.syncMode.full": "full history",
    "connections.syncMode.incremental": "increments",

    // Pagination
    "connections.pagination.label": "Pagination",
    "connections.pagination.prev": "← Previous",
    "connections.pagination.next": "Next →",

    // Shared dialog fields / buttons
    "connections.save": "Save",
    "connections.cancel": "Cancel",
    "connections.close": "Close",
    "connections.testAction": "Test",
    "connections.continue": "Continue",
    "connections.back": "Back",
    "connections.connectedAs": "Connected as {name}",
    "connections.connectionFailed": "Connection failed",
    "connections.saveFailed": "Failed to save",

    // Field labels
    "connections.field.name": "Connection name",
    "connections.field.jiraUrl": "Jira base URL",
    "connections.field.atlassianEmail": "Atlassian account email",
    "connections.field.jiraToken": "Jira API token",
    "connections.field.freeloEmail": "Freelo email",
    "connections.field.freeloApiKey": "Freelo API key",
    "connections.field.freeloApiUrl": "Freelo API URL",

    // Field placeholders
    "connections.placeholder.name": "e.g. SAB, Client X, …",
    "connections.placeholder.nameShort": "e.g. SAB, Client X",
    "connections.placeholder.token": "paste your token",
    "connections.placeholder.apiKey": "paste your API key",

    // Custom / self-hosted server checkbox
    "connections.customHost.title": "Custom / self-hosted server",
    "connections.customHost.descriptionPrefix": "Enable for on-premise Jira outside ",
    "connections.customHost.descriptionSuffix":
      ". Verify the URL is trustworthy — the token is sent to this server.",

    // Advanced settings toggle
    "connections.hideAdvanced": "Hide advanced settings",
    "connections.showAdvanced": "Show advanced settings",

    // AddConnectionDialog — headers
    "connections.add.dialogLabel": "Add new connection",
    "connections.add.chooseProvider": "Choose a provider",
    "connections.add.signIn": "sign-in",
    "connections.add.chooseProjects": "Choose projects",

    // EditConnectionDialog
    "connections.edit.dialogLabel": "Edit {name}",
    "connections.edit.title": "Edit {provider} connection",
    "connections.edit.replaceToken": "Replace API token",
    "connections.edit.replaceApiKey": "Replace API key",
    "connections.edit.newToken": "New Jira API token",
    "connections.edit.newApiKey": "New Freelo API key",
    "connections.edit.statusesLoadFailed": "Failed to load statuses",

    // EditConnectionDialog — dashboard
    "connections.dashboard.title": "Show dashboard",
    "connections.dashboard.description":
      "Adds this Jira to the “Jira overview” table in the menu. Requires the JQL filter below.",
    "connections.dashboard.jqlLabel": "JQL for Dashboard",
    "connections.dashboard.jqlHint":
      "Atlassian rejects JQL without at least one restriction. Without ORDER BY it uses Jira's default ordering.",

    // EditConnectionDialog — auto transition
    "connections.autoTransition.summary": "Automatic status transition (optional)",
    "connections.autoTransition.fromLabel": "When an issue is in status…",
    "connections.autoTransition.toLabel": "…transition on start to",
    "connections.autoTransition.hint":
      "Silent best-effort action — if no direct transition exists between the selected statuses in Jira, Tracker simply won't attempt it (it logs instead). If you leave “—” selected, nothing happens.",

    // EditConnectionDialog — color
    "connections.color.title": "Custom color in Reports",
    "connections.color.description":
      "When disabled, the provider's default color is used.",
    "connections.color.pickerLabel": "Color for this connection",

    // StatusSelect
    "connections.status.none": "— not selected —",
    "connections.status.loading": "Loading…",
  },
} as const;

/** Layout / chrome strings — AppShell, StartTrackingBar, CommandBar, SyncBanner, UpdateBanner. */
export const layout = {
  cs: {
    // AppShell — toasts / messages
    "layout.reindexed": "Reindexováno {count} {noun}.",
    "layout.reindexFailed": "Reindexace selhala.",
    "layout.worklogSaved": "Uloženo {dur} na {issueKey}.",
    "layout.syncWithJiraFailed": "Synchronizace s Jirou selhala",
    "layout.worklogError": "Záznam: {msg}",
    "layout.idleStopFailed": "Idle: stop selhal",
    "layout.idleRestartFailed": "Idle: restart selhal",
    "layout.failedToSaveWorklog": "Failed to save worklog",
    "layout.startTimerFailed": "Nepodařilo se spustit časomíru",
    "layout.unassignedTimerRunning":
      "Časomíra běží bez přiřazeného úkolu — nezapomeňte ho přiřadit před uložením.",
    "layout.timerReassigned": "Časomíra přepnuta na {issueKey}.",
    "layout.reassignFailed": "Přepnutí úkolu selhalo.",
    "layout.entryAdded": "Záznam přidán na {issueKey}.",
    "layout.entrySaveFailed": "Záznam se nepodařilo uložit",
    "layout.timerDiscarded": "Časomíra zahozena bez uložení.",
    "layout.timerDiscardFailed": "Zahození časomíry selhalo.",

    // StartTrackingBar
    "layout.start": "Spustit",
    "layout.startUnassigned": "Spustit bez úkolu",
    "layout.startTitleIssue": "Spustit časomíru pro označený úkol",
    "layout.startTitleUnassigned":
      "Spustit časomíru bez úkolu — můžete přiřadit později",
    "layout.startTitleSearch": "Vyhledejte úkol nebo zadejte poznámku",
    "layout.searchPlaceholder": "Začít stopovat…",
    "layout.searchAriaLabel": "Vyhledat a spustit časomíru",
    "layout.commentPlaceholder": "Poznámka (volitelné)",
    "layout.commentAriaLabel": "Poznámka k zapnuté časomíře",
    "layout.favorites": "★ Oblíbené",
    "layout.loading": "Načítám…",
    "layout.searching": "Vyhledávání…",
    "layout.typeToSearch": "Začněte psát pro vyhledání úkolu.",
    "layout.noMatchingIssues": "Žádné odpovídající úkoly.",
    "layout.recentlyTracked": "Naposledy trackováno",
    "layout.summaryLoading": "(načítá se…)",
    "layout.changeRunningIssue": "Změnit úkol běžící časomíry",
    "layout.unassignedChip": "⚠ BEZ ÚKOLU",
    "layout.commentShort": "Poznámka",
    "layout.editComment": "Upravit poznámku",
    "layout.assignBeforeSaving": "Přiřaďte úkol před uložením",
    "layout.addComment": "+ poznámka",
    "layout.stop": "Stop",

    // CommandBar
    "layout.cmdSettings": "Nastavení",
    "layout.cmdRefresh": "Obnovit",
    "layout.cmdReindex": "Reindexovat",
    "layout.cmdNewEntry": "Nový záznam",

    // SyncBanner
    "layout.phaseConnection": "Připojuji se",
    "layout.phaseIssues": "Načítám úkoly",
    "layout.phaseWorklogs": "Načítám záznamy",
    "layout.syncFailed": "Synchronizace selhala",
    "layout.syncComplete": "Synchronizace dokončena",
    "layout.syncing": "Synchronizuji {name}",
    "layout.syncingGeneric": "Synchronizuji…",
    "layout.close": "Zavřít",

    // UpdateBanner
    "layout.updateAvailable": "Je dostupná nová verze{version}.",
    "layout.download": "Stáhnout",
    "layout.later": "Později",
    "layout.downloadingUpdate": "Stahuji aktualizaci{version}…{pct}",
    "layout.updateReady":
      "Aktualizace{version} je stažená a připravená.{timerNote}",
    "layout.updateTimerNote": " Běží časomíra — po restartu bude pokračovat.",
    "layout.restartAndFinish": "Restartovat a dokončit",
    "layout.updateFailed": "Aktualizace se nezdařila{error}",
  },
  en: {
    // AppShell — toasts / messages
    "layout.reindexed": "Reindexed {count} issues.",
    "layout.reindexFailed": "Reindex failed.",
    "layout.worklogSaved": "Saved {dur} on {issueKey}.",
    "layout.syncWithJiraFailed": "Sync with Jira failed",
    "layout.worklogError": "Entry: {msg}",
    "layout.idleStopFailed": "Idle: stop failed",
    "layout.idleRestartFailed": "Idle: restart failed",
    "layout.failedToSaveWorklog": "Failed to save worklog",
    "layout.startTimerFailed": "Could not start the timer",
    "layout.unassignedTimerRunning":
      "The timer is running with no assigned issue — remember to assign one before saving.",
    "layout.timerReassigned": "Timer switched to {issueKey}.",
    "layout.reassignFailed": "Failed to switch the issue.",
    "layout.entryAdded": "Entry added to {issueKey}.",
    "layout.entrySaveFailed": "Failed to save the entry",
    "layout.timerDiscarded": "Timer discarded without saving.",
    "layout.timerDiscardFailed": "Failed to discard the timer.",

    // StartTrackingBar
    "layout.start": "Start",
    "layout.startUnassigned": "Start without an issue",
    "layout.startTitleIssue": "Start the timer for the selected issue",
    "layout.startTitleUnassigned":
      "Start the timer without an issue — you can assign one later",
    "layout.startTitleSearch": "Search for an issue or enter a note",
    "layout.searchPlaceholder": "Start tracking…",
    "layout.searchAriaLabel": "Search and start the timer",
    "layout.commentPlaceholder": "Note (optional)",
    "layout.commentAriaLabel": "Note for the running timer",
    "layout.favorites": "★ Favorites",
    "layout.loading": "Loading…",
    "layout.searching": "Searching…",
    "layout.typeToSearch": "Start typing to search for an issue.",
    "layout.noMatchingIssues": "No matching issues.",
    "layout.recentlyTracked": "Recently tracked",
    "layout.summaryLoading": "(loading…)",
    "layout.changeRunningIssue": "Change the running timer's issue",
    "layout.unassignedChip": "⚠ NO ISSUE",
    "layout.commentShort": "Note",
    "layout.editComment": "Edit note",
    "layout.assignBeforeSaving": "Assign an issue before saving",
    "layout.addComment": "+ note",
    "layout.stop": "Stop",

    // CommandBar
    "layout.cmdSettings": "Settings",
    "layout.cmdRefresh": "Refresh",
    "layout.cmdReindex": "Reindex",
    "layout.cmdNewEntry": "New entry",

    // SyncBanner
    "layout.phaseConnection": "Connecting",
    "layout.phaseIssues": "Loading issues",
    "layout.phaseWorklogs": "Loading records",
    "layout.syncFailed": "Sync failed",
    "layout.syncComplete": "Sync complete",
    "layout.syncing": "Syncing {name}",
    "layout.syncingGeneric": "Syncing…",
    "layout.close": "Close",

    // UpdateBanner
    "layout.updateAvailable": "A new version{version} is available.",
    "layout.download": "Download",
    "layout.later": "Later",
    "layout.downloadingUpdate": "Downloading update{version}…{pct}",
    "layout.updateReady":
      "Update{version} has been downloaded and is ready.{timerNote}",
    "layout.updateTimerNote": " A timer is running — it will resume after restart.",
    "layout.restartAndFinish": "Restart and finish",
    "layout.updateFailed": "The update failed{error}",
  },
} as const;

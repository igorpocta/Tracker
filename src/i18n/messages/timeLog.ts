/** Time log (Časový záznam) + Unassigned (Nepřiřazené) route strings. */
export const timeLog = {
  cs: {
    // TimeLog — header
    "timeLog.title": "Časový záznam",
    "timeLog.mode.aria": "Režim",
    "timeLog.mode.day": "Den",
    "timeLog.mode.week": "Týden",
    "timeLog.nav.prevWeek": "Předchozí týden",
    "timeLog.nav.prevDay": "Předchozí den",
    "timeLog.nav.nextWeek": "Další týden",
    "timeLog.nav.nextDay": "Další den",
    "timeLog.nav.today": "Dnes",
    "timeLog.total": "Celkem",

    // TimeLog — day header relative prefixes
    "timeLog.header.today": "Dnes",
    "timeLog.header.yesterday": "Včera",
    "timeLog.header.tomorrow": "Zítra",

    // TimeLog — rows / empty / loading
    "timeLog.loading": "Načítání…",
    "timeLog.empty.prefix": "Pro toto období nejsou žádné záznamy. Stiskněte",
    "timeLog.empty.suffix": "pro přidání.",
    "timeLog.newEntry": "Nový záznam",

    // TimeLog — toasts / errors
    "timeLog.error.deleteRecord": "Nepodařilo se smazat záznam",
    "timeLog.error.deleteFailed": "Záznam se nepodařilo smazat",
    "timeLog.deleted": "Záznam smazán",
    "timeLog.undo": "Vrátit",
    "timeLog.error.missingLocalId": "Chybí lokální id záznamu",
    "timeLog.error.restoreFailed": "Obnovení záznamu selhalo.",
    "timeLog.error.noId": "Záznam nemá ID, nelze upravit.",
    "timeLog.error.updateFailed": "Záznam se nepodařilo aktualizovat",
    "timeLog.error.createFailed": "Nepodařilo se vytvořit záznam",
    "timeLog.error.splitFailed": "Split selhal",

    // WorklogRow
    "timeLog.row.commentPlaceholder": "Komentář",
    "timeLog.row.editComment": "Upravit komentář",
    "timeLog.row.loadingSummary": "(načítá se…)",
    "timeLog.row.unassigned": "Nepřiřazen",
    "timeLog.row.synced": "Synchronizováno s providerem.",
    "timeLog.row.syncFailed": "Synchronizace s providerem selhala.",
    "timeLog.row.forceSync": "Klikni pro vynucenou synchronizaci s providerem",
    "timeLog.row.localChip": "⚠ lokální · ↻",
    "timeLog.row.pendingAssignment":
      "Časomíra byla zastavena bez přiřazeného úkolu — vyberte úkol vlevo",
    "timeLog.row.noIssueChip": "⚠ bez úkolu",
    "timeLog.row.editStart": "Upravit začátek",
    "timeLog.row.editEnd": "Upravit konec",
    "timeLog.row.startAria": "Začátek",
    "timeLog.row.endAria": "Konec",
    "timeLog.row.editDuration": "Upravit trvání",
    "timeLog.row.durationAria": "Trvání",
    "timeLog.row.deleteAria": "Smazat záznam {issueKey}",
    "timeLog.row.delete": "Smazat",

    // SplitWorklogDialog
    "timeLog.split.aria": "Rozdělit záznam",
    "timeLog.split.title": "Rozdělit záznam v {time}",
    "timeLog.split.descriptionPrefix": "První kus zůstane na úkolu",
    "timeLog.split.noIssue": "(bez úkolu)",
    "timeLog.split.descriptionSuffix":
      "Druhý kus přiřaď k jinému úkolu (nech prázdné pro 'bez úkolu').",
    "timeLog.split.cancel": "Zrušit",
    "timeLog.split.confirm": "Rozdělit",

    // CreateWorklogDialog
    "timeLog.create.aria": "Vytvořit záznam z časové osy",
    "timeLog.create.title": "Vytvořit záznam {start}–{end}",
    "timeLog.create.minutes": "{count} min",
    "timeLog.create.description":
      "Zadej úkol — záznam bude rovnou odeslán do providera (Jira / Freelo podle prefixu klíče).",
    "timeLog.create.cancel": "Zrušit",
    "timeLog.create.confirm": "Vytvořit",

    // Unassigned
    "unassigned.title": "Nepřiřazené",
    "unassigned.count.one": "záznam",
    "unassigned.count.few": "záznamy",
    "unassigned.count.many": "záznamů",
    "unassigned.toAssign": "k přiřazení",
    "unassigned.description":
      "Záznamy bez úkolu se nevyfakturují. Přiřaď je dřív, než budeš dělat fakturu.",
    "unassigned.loading": "Načítám…",
    "unassigned.allAssigned": "Vše přiřazeno 🎉",
    "unassigned.emptyHint": "Žádné nepřiřazené záznamy — nic ti na faktuře neuteče.",
    "unassigned.noNote": "(bez poznámky)",
  },
  en: {
    // TimeLog — header
    "timeLog.title": "Time log",
    "timeLog.mode.aria": "Mode",
    "timeLog.mode.day": "Day",
    "timeLog.mode.week": "Week",
    "timeLog.nav.prevWeek": "Previous week",
    "timeLog.nav.prevDay": "Previous day",
    "timeLog.nav.nextWeek": "Next week",
    "timeLog.nav.nextDay": "Next day",
    "timeLog.nav.today": "Today",
    "timeLog.total": "Total",

    // TimeLog — day header relative prefixes
    "timeLog.header.today": "Today",
    "timeLog.header.yesterday": "Yesterday",
    "timeLog.header.tomorrow": "Tomorrow",

    // TimeLog — rows / empty / loading
    "timeLog.loading": "Loading…",
    "timeLog.empty.prefix": "There are no entries for this period. Press",
    "timeLog.empty.suffix": "to add one.",
    "timeLog.newEntry": "New entry",

    // TimeLog — toasts / errors
    "timeLog.error.deleteRecord": "Could not delete the entry",
    "timeLog.error.deleteFailed": "The entry could not be deleted",
    "timeLog.deleted": "Entry deleted",
    "timeLog.undo": "Undo",
    "timeLog.error.missingLocalId": "Missing local entry id",
    "timeLog.error.restoreFailed": "Restoring the entry failed.",
    "timeLog.error.noId": "The entry has no ID and cannot be edited.",
    "timeLog.error.updateFailed": "The entry could not be updated",
    "timeLog.error.createFailed": "Could not create the entry",
    "timeLog.error.splitFailed": "Split failed",

    // WorklogRow
    "timeLog.row.commentPlaceholder": "Comment",
    "timeLog.row.editComment": "Edit comment",
    "timeLog.row.loadingSummary": "(loading…)",
    "timeLog.row.unassigned": "Unassigned",
    "timeLog.row.synced": "Synced with the provider.",
    "timeLog.row.syncFailed": "Sync with the provider failed.",
    "timeLog.row.forceSync": "Click to force sync with the provider",
    "timeLog.row.localChip": "⚠ local · ↻",
    "timeLog.row.pendingAssignment":
      "The timer was stopped without an assigned issue — pick an issue on the left",
    "timeLog.row.noIssueChip": "⚠ no issue",
    "timeLog.row.editStart": "Edit start",
    "timeLog.row.editEnd": "Edit end",
    "timeLog.row.startAria": "Start",
    "timeLog.row.endAria": "End",
    "timeLog.row.editDuration": "Edit duration",
    "timeLog.row.durationAria": "Duration",
    "timeLog.row.deleteAria": "Delete entry {issueKey}",
    "timeLog.row.delete": "Delete",

    // SplitWorklogDialog
    "timeLog.split.aria": "Split entry",
    "timeLog.split.title": "Split entry at {time}",
    "timeLog.split.descriptionPrefix": "The first part stays on issue",
    "timeLog.split.noIssue": "(no issue)",
    "timeLog.split.descriptionSuffix":
      "Assign the second part to another issue (leave empty for 'no issue').",
    "timeLog.split.cancel": "Cancel",
    "timeLog.split.confirm": "Split",

    // CreateWorklogDialog
    "timeLog.create.aria": "Create entry from the timeline",
    "timeLog.create.title": "Create entry {start}–{end}",
    "timeLog.create.minutes": "{count} min",
    "timeLog.create.description":
      "Enter an issue — the entry will be sent straight to the provider (Jira / Freelo based on the key prefix).",
    "timeLog.create.cancel": "Cancel",
    "timeLog.create.confirm": "Create",

    // Unassigned
    "unassigned.title": "Unassigned",
    "unassigned.count.one": "entry",
    "unassigned.count.few": "entries",
    "unassigned.count.many": "entries",
    "unassigned.toAssign": "to assign",
    "unassigned.description":
      "Entries without an issue won't be invoiced. Assign them before you prepare the invoice.",
    "unassigned.loading": "Loading…",
    "unassigned.allAssigned": "All assigned 🎉",
    "unassigned.emptyHint": "No unassigned entries — nothing will slip off your invoice.",
    "unassigned.noNote": "(no note)",
  },
} as const;

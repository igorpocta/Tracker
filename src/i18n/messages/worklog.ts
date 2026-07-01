/** Worklog / timer-widget strings (StartTimeEditor, SuggestionBanner, Timer, CommentInput, IssuePicker, assign-worklog). */
export const worklog = {
  cs: {
    // StartTimeEditor
    "worklog.startEditor.label": "Začátek",
    "worklog.startEditor.minus5": "Odečíst 5 minut",
    "worklog.startEditor.minus1": "Odečíst 1 minutu",
    "worklog.startEditor.timeInput": "Čas začátku",
    "worklog.startEditor.plus1": "Přidat 1 minutu",
    "worklog.startEditor.plus5": "Přidat 5 minut",
    "worklog.startEditor.resetNow": "Resetovat začátek na teď",
    "worklog.startEditor.now": "Teď",

    // SuggestionBanner
    "worklog.suggestion.startFailed": "Nepodařilo se spustit časomíru.",
    "worklog.suggestion.start": "Začít",
    "worklog.suggestion.context": "(před tím v {hour} {count}× v posledních 14 dnech)",
    "worklog.suggestion.run": "Spustit",
    "worklog.suggestion.dismissTitle": "Skrýt pro dnešek",
    "worklog.suggestion.dismiss": "Skrýt",

    // Timer
    "worklog.timer.elapsed": "Uplynulý čas {duration}",
    "worklog.timer.notRunning": "Časomíra neběží",

    // CommentInput
    "worklog.comment.placeholder": "Co jste dělal/a? (volitelné)",
    "worklog.comment.label": "Komentář",

    // IssuePicker
    "worklog.picker.assignTitle": "Přiřadit úkol k záznamu",
    "worklog.picker.assign": "Přiřadit úkol",
    "worklog.picker.searchPlaceholder": "Hledat úkol…",
    "worklog.picker.recent": "Naposledy trackováno",
    "worklog.picker.startTyping": "Začni psát pro vyhledání úkolu.",
    "worklog.picker.noMatches": "Žádné odpovídající úkoly.",
    "worklog.picker.noName": "(bez názvu)",

    // useAssignWorklog
    "worklog.assign.success": "Záznam přiřazen na {issueKey}.",
    "worklog.assign.error": "Přiřazení úkolu selhalo.",
  },
  en: {
    // StartTimeEditor
    "worklog.startEditor.label": "Start",
    "worklog.startEditor.minus5": "Subtract 5 minutes",
    "worklog.startEditor.minus1": "Subtract 1 minute",
    "worklog.startEditor.timeInput": "Start time",
    "worklog.startEditor.plus1": "Add 1 minute",
    "worklog.startEditor.plus5": "Add 5 minutes",
    "worklog.startEditor.resetNow": "Reset start to now",
    "worklog.startEditor.now": "Now",

    // SuggestionBanner
    "worklog.suggestion.startFailed": "Couldn't start the timer.",
    "worklog.suggestion.start": "Start",
    "worklog.suggestion.context": "(previously at {hour}, {count}× in the last 14 days)",
    "worklog.suggestion.run": "Start",
    "worklog.suggestion.dismissTitle": "Hide for today",
    "worklog.suggestion.dismiss": "Hide",

    // Timer
    "worklog.timer.elapsed": "Elapsed time {duration}",
    "worklog.timer.notRunning": "Timer not running",

    // CommentInput
    "worklog.comment.placeholder": "What were you working on? (optional)",
    "worklog.comment.label": "Comment",

    // IssuePicker
    "worklog.picker.assignTitle": "Assign an issue to this entry",
    "worklog.picker.assign": "Assign issue",
    "worklog.picker.searchPlaceholder": "Search issue…",
    "worklog.picker.recent": "Recently tracked",
    "worklog.picker.startTyping": "Start typing to search for an issue.",
    "worklog.picker.noMatches": "No matching issues.",
    "worklog.picker.noName": "(no title)",

    // useAssignWorklog
    "worklog.assign.success": "Entry assigned to {issueKey}.",
    "worklog.assign.error": "Failed to assign the issue.",
  },
} as const;

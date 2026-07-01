/** Timer / day-timeline / idle / add-entry strings. */
export const timer = {
  cs: {
    // DayTimeline
    "timer.timeline.label": "Časová osa dne",
    "timer.timeline.heading": "Časová osa dne",
    "timer.timeline.noIssue": "(bez úkolu)",
    "timer.timeline.hint":
      "Klikni pro zvýraznění. Tažením přes prázdné místo založíš nový záznam — čas u kurzoru ukazuje začátek, konec a dobu trvání. U lokálních záznamů můžeš tažením rozdělit blok na dva úkoly.",

    // IdleDialog
    "timer.idle.label": "Detekována ne-aktivita",
    "timer.idle.title": "Byl jsi pryč {mins} {unit}",
    "timer.idle.unit.one": "minutu",
    "timer.idle.unit.few": "minuty",
    "timer.idle.unit.many": "minut",
    "timer.idle.body":
      "Časomíra na {issue} běžela celou dobu. Co s tou ne-aktivitou?",
    "timer.idle.noIssue": "úkolu bez přiřazení",
    "timer.idle.keep": "Zachovat",
    "timer.idle.keep.desc": "Časomíra pokračuje, čas se započítá.",
    "timer.idle.discard": "Odečíst a zastavit",
    "timer.idle.discard.desc": "Uložit worklog s časem před ne-aktivitou.",
    "timer.idle.discardContinue": "Odečíst a pokračovat",
    "timer.idle.discardContinue.desc":
      "Zastaví, uloží, znovu spustí časomíru pro stejný úkol.",

    // StopDialog (TimerControls)
    "timer.stop.label": "Zastavit časomíru",
    "timer.stop.title": "Zastavit a uložit záznam",
    "timer.stop.edited": "upraveno",
    "timer.stop.editStart": "Upravit čas začátku",
    "timer.stop.discardTitle": "Zruší časomíru bez uložení worklogu",
    "timer.stop.discard": "Zahodit záznam",
    "timer.stop.close": "Zavřít",
    "timer.stop.confirm": "Zastavit a uložit",

    // AddEntryPanel
    "timer.add.endsNextDay": "konec další den",
    "timer.add.error.noIssue": "Nejprve vyberte úkol.",
    "timer.add.error.endBeforeStart": "Konec musí být po začátku.",
    "timer.add.error.saveFailed": "Záznam se nepodařilo uložit",
    "timer.add.label": "Přidat záznam",
    "timer.add.heading": "Přidat záznam",
    "timer.add.subtitle": "Zaznamenat strávený čas",
    "timer.add.closeLabel": "Zavřít panel přidání záznamu",
    "timer.add.issue": "Úkol",
    "timer.add.issuePlaceholder": "Vyhledat psaním",
    "timer.add.date": "Datum",
    "timer.add.startEnd": "Začátek a konec",
    "timer.add.start": "Začátek",
    "timer.add.end": "Konec",
    "timer.add.comment": "Komentář (volitelné)",
    "timer.add.total": "Celkem",
    "timer.add.save": "Uložit záznam",
    "timer.add.issueLoading": "(načítá se…)",
  },
  en: {
    // DayTimeline
    "timer.timeline.label": "Day timeline",
    "timer.timeline.heading": "Day timeline",
    "timer.timeline.noIssue": "(no issue)",
    "timer.timeline.hint":
      "Click to highlight. Drag across an empty spot to create a new entry — the time at the cursor shows the start, end and duration. For local entries you can drag to split a block into two issues.",

    // IdleDialog
    "timer.idle.label": "Idle detected",
    "timer.idle.title": "You were away {mins} {unit}",
    "timer.idle.unit.one": "minute",
    "timer.idle.unit.few": "minutes",
    "timer.idle.unit.many": "minutes",
    "timer.idle.body":
      "The timer on {issue} kept running the whole time. What do you want to do with the idle period?",
    "timer.idle.noIssue": "an unassigned issue",
    "timer.idle.keep": "Keep",
    "timer.idle.keep.desc": "The timer continues, the time is counted.",
    "timer.idle.discard": "Discard and stop",
    "timer.idle.discard.desc": "Save the worklog with the time before the idle period.",
    "timer.idle.discardContinue": "Discard and continue",
    "timer.idle.discardContinue.desc":
      "Stops, saves, and restarts the timer for the same issue.",

    // StopDialog (TimerControls)
    "timer.stop.label": "Stop the timer",
    "timer.stop.title": "Stop and save the entry",
    "timer.stop.edited": "edited",
    "timer.stop.editStart": "Edit start time",
    "timer.stop.discardTitle": "Discards the timer without saving a worklog",
    "timer.stop.discard": "Discard entry",
    "timer.stop.close": "Close",
    "timer.stop.confirm": "Stop and save",

    // AddEntryPanel
    "timer.add.endsNextDay": "ends next day",
    "timer.add.error.noIssue": "Choose an issue first.",
    "timer.add.error.endBeforeStart": "End must be after the start.",
    "timer.add.error.saveFailed": "Failed to save the entry",
    "timer.add.label": "Add entry",
    "timer.add.heading": "Add entry",
    "timer.add.subtitle": "Log time you've spent",
    "timer.add.closeLabel": "Close the add-entry panel",
    "timer.add.issue": "Issue",
    "timer.add.issuePlaceholder": "Type to search",
    "timer.add.date": "Date",
    "timer.add.startEnd": "Start and end",
    "timer.add.start": "Start",
    "timer.add.end": "End",
    "timer.add.comment": "Comment (optional)",
    "timer.add.total": "Total",
    "timer.add.save": "Save entry",
    "timer.add.issueLoading": "(loading…)",
  },
} as const;

/**
 * Miscellaneous UI strings migrated file-by-file: Reports summary cards, the
 * Settings route shell (tab labels), the Goals month-end prediction card, the
 * Calendar cell context menu and the favorite-issue star toggle.
 */
export const misc = {
  cs: {
    // Reports — SummaryCards
    "misc.summary.totalTime": "Celkový čas",
    "misc.summary.daysWorked": "Odpracovaných dní",
    "misc.summary.issuesTouched": "Dotčených úkolů",
    "misc.summary.earnings": "Výdělek",
    "misc.summary.hideEarnings": "Skrýt výdělek",
    "misc.summary.showEarnings": "Zobrazit výdělek",

    // Settings route shell — sidebar + tab labels
    "misc.settings.navAria": "Sekce nastavení",
    "misc.settings.title": "Nastavení",
    "misc.settings.tab.connection": "Připojení",
    "misc.settings.tab.general": "Obecné",
    "misc.settings.tab.reporting": "Reporting",
    "misc.settings.tab.goals": "Cíle",
    "misc.settings.tab.focus": "Focus",
    "misc.settings.tab.appearance": "Vzhled",
    "misc.settings.tab.about": "O aplikaci",

    // Goals — GoalsPrediction
    "misc.prediction.heading": "Predikce konce měsíce",
    "misc.prediction.avgLabel": "Průměr letošního měsíce",
    "misc.prediction.avgValue": "{duration} / den",
    "misc.prediction.remainingLabel": "Zbývá",
    "misc.prediction.remainingValue": "{days} pracovních dnů",
    "misc.prediction.predictionLabel": "Predikce",
    "misc.prediction.goalSuffix": "(cíl {duration})",
    "misc.prediction.paceLabel": "Tempo",
    "misc.prediction.onTarget": "přesně na cíl",
    "misc.prediction.under": "budete {duration} pod cílem",
    "misc.prediction.over": "budete {duration} nad cílem",

    // Calendar — CellContextMenu
    "misc.cellMenu.aria": "Akce pro den",
    "misc.cellMenu.markWorking": "Označit jako pracovní den",
    "misc.cellMenu.markNonWorking": "Označit jako nepracovní den",
    "misc.cellMenu.dayDetail": "Detail dne",
    "misc.cellMenu.reason": "Důvod",
    "misc.cellMenu.reason.vacation": "Dovolená",
    "misc.cellMenu.reason.holiday": "Svátek",
    "misc.cellMenu.reason.personal": "Osobní",
    "misc.cellMenu.back": "← Zpět",

    // Favorites — FavoriteStar
    "misc.favorite.remove": "Odebrat z oblíbených",
    "misc.favorite.add": "Přidat do oblíbených",
  },
  en: {
    // Reports — SummaryCards
    "misc.summary.totalTime": "Total time",
    "misc.summary.daysWorked": "Days worked",
    "misc.summary.issuesTouched": "Issues touched",
    "misc.summary.earnings": "Earnings",
    "misc.summary.hideEarnings": "Hide earnings",
    "misc.summary.showEarnings": "Show earnings",

    // Settings route shell — sidebar + tab labels
    "misc.settings.navAria": "Settings sections",
    "misc.settings.title": "Settings",
    "misc.settings.tab.connection": "Connections",
    "misc.settings.tab.general": "General",
    "misc.settings.tab.reporting": "Reporting",
    "misc.settings.tab.goals": "Goals",
    "misc.settings.tab.focus": "Focus",
    "misc.settings.tab.appearance": "Appearance",
    "misc.settings.tab.about": "About",

    // Goals — GoalsPrediction
    "misc.prediction.heading": "Month-end prediction",
    "misc.prediction.avgLabel": "Average this month",
    "misc.prediction.avgValue": "{duration} / day",
    "misc.prediction.remainingLabel": "Remaining",
    "misc.prediction.remainingValue": "{days} working days",
    "misc.prediction.predictionLabel": "Prediction",
    "misc.prediction.goalSuffix": "(goal {duration})",
    "misc.prediction.paceLabel": "Pace",
    "misc.prediction.onTarget": "exactly on target",
    "misc.prediction.under": "you'll be {duration} under the goal",
    "misc.prediction.over": "you'll be {duration} over the goal",

    // Calendar — CellContextMenu
    "misc.cellMenu.aria": "Day actions",
    "misc.cellMenu.markWorking": "Mark as working day",
    "misc.cellMenu.markNonWorking": "Mark as non-working day",
    "misc.cellMenu.dayDetail": "Day detail",
    "misc.cellMenu.reason": "Reason",
    "misc.cellMenu.reason.vacation": "Vacation",
    "misc.cellMenu.reason.holiday": "Holiday",
    "misc.cellMenu.reason.personal": "Personal",
    "misc.cellMenu.back": "← Back",

    // Favorites — FavoriteStar
    "misc.favorite.remove": "Remove from favorites",
    "misc.favorite.add": "Add to favorites",
  },
} as const;

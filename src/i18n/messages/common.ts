/** Common shared UI strings (toasts, error boundary, confirm, titlebar, shortcut, pomodoro). */
export const common = {
  cs: {
    // Toast
    "common.toast.close": "Zavřít",
    // ErrorBoundary
    "common.error.title": "Něco se nepovedlo",
    "common.error.body":
      "Tracker narazil na neočekávanou chybu a nemůže pokračovat. Načtení okna obvykle pomůže.",
    "common.error.tryAgain": "Zkusit znovu",
    "common.error.reload": "Načíst znovu",
    // ConfirmButton
    "common.confirm.confirm": "Potvrdit",
    "common.confirm.cancel": "Zrušit",
    // WindowTitlebar
    "common.titlebar.label": "Titulní lišta okna",
    "common.titlebar.minimize": "Minimalizovat",
    "common.titlebar.restore": "Obnovit",
    "common.titlebar.maximize": "Maximalizovat",
    "common.titlebar.close": "Zavřít",
    // GlobalShortcutSetting
    "common.shortcut.taken":
      "Zkratka je nejspíš obsazená jinou aplikací — zkuste jinou.",
    "common.shortcut.saveFailed": "Zkratku se nepodařilo uložit.",
    "common.shortcut.press": "Stiskněte kombinaci…",
    "common.shortcut.disabled": "Vypnuto",
    "common.shortcut.cancel": "Zrušit",
    "common.shortcut.change": "Změnit",
    "common.shortcut.resetDefault": "Obnovit výchozí",
    "common.shortcut.disable": "Vypnout",
    "common.shortcut.notActive":
      "Zkratka není aktivní — nejspíš ji drží jiná aplikace. Zvolte jinou kombinaci.",
    "common.shortcut.hint":
      "Spustí nebo zastaví časovač odkudkoli v systému — i když je okno Trackeru skryté. Při spuštění bez úkolu se založí nepřiřazený záznam.",
    // Pomodoro notifications
    "common.pomodoro.breakTitle": "Pomodoro · čas na pauzu",
    "common.pomodoro.breakBody":
      "Dokončen {workMin}min cyklus. Odpočiň {breakMin} min.",
    "common.pomodoro.workTitle": "Pomodoro · zpět do práce",
    "common.pomodoro.workBody":
      "Konec pauzy. Další {workMin}min cyklus se nepustí sám — kdyžtak prodluž.",
  },
  en: {
    // Toast
    "common.toast.close": "Close",
    // ErrorBoundary
    "common.error.title": "Something went wrong",
    "common.error.body":
      "Tracker ran into an unexpected error and can't continue. Reloading the window usually helps.",
    "common.error.tryAgain": "Try again",
    "common.error.reload": "Reload",
    // ConfirmButton
    "common.confirm.confirm": "Confirm",
    "common.confirm.cancel": "Cancel",
    // WindowTitlebar
    "common.titlebar.label": "Window title bar",
    "common.titlebar.minimize": "Minimize",
    "common.titlebar.restore": "Restore",
    "common.titlebar.maximize": "Maximize",
    "common.titlebar.close": "Close",
    // GlobalShortcutSetting
    "common.shortcut.taken":
      "The shortcut is probably taken by another app — try a different one.",
    "common.shortcut.saveFailed": "Couldn't save the shortcut.",
    "common.shortcut.press": "Press a shortcut…",
    "common.shortcut.disabled": "Disabled",
    "common.shortcut.cancel": "Cancel",
    "common.shortcut.change": "Change",
    "common.shortcut.resetDefault": "Reset to default",
    "common.shortcut.disable": "Disable",
    "common.shortcut.notActive":
      "The shortcut is not active — another app is probably holding it. Pick a different combination.",
    "common.shortcut.hint":
      "Starts or stops the timer from anywhere in the system — even when the Tracker window is hidden. Starting without a task creates an unassigned entry.",
    // Pomodoro notifications
    "common.pomodoro.breakTitle": "Pomodoro · time for a break",
    "common.pomodoro.breakBody":
      "Finished a {workMin}min cycle. Rest for {breakMin} min.",
    "common.pomodoro.workTitle": "Pomodoro · back to work",
    "common.pomodoro.workBody":
      "Break's over. The next {workMin}min cycle won't start by itself — extend it if you need to.",
  },
} as const;

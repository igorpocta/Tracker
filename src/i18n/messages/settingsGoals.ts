/** Settings → Cíle / working week / non-working days strings. */
export const settingsGoals = {
  cs: {
    "settingsGoals.heading": "Cíle",

    "settingsGoals.dailyGoal.title": "Denní cíl hodin",
    "settingsGoals.dailyGoal.description":
      "Kolik hodin chcete denně odpracovat. Používá se v sekci Cíle.",

    "settingsGoals.pomodoro.title": "Pomodoro",
    "settingsGoals.pomodoro.enable": "Zapnout Pomodoro",
    "settingsGoals.pomodoro.description":
      "Při běžícím timeru ti aplikace pošle notifikaci po dokončení work cyklu a poté znovu po pauze. Cyklus se nikam neukládá — slouží jen jako připomínka.",
    "settingsGoals.pomodoro.work": "Práce (min)",
    "settingsGoals.pomodoro.break": "Pauza (min)",

    "settingsGoals.workingWeek.title": "Pracovní dny v týdnu",
    "settingsGoals.workingWeek.description":
      "Které dny v týdnu obvykle pracujete. Víkendy a státní svátky se nezapočítávají do cílů.",
    "settingsGoals.weekday.mon": "Pondělí",
    "settingsGoals.weekday.tue": "Úterý",
    "settingsGoals.weekday.wed": "Středa",
    "settingsGoals.weekday.thu": "Čtvrtek",
    "settingsGoals.weekday.fri": "Pátek",
    "settingsGoals.weekday.sat": "Sobota",
    "settingsGoals.weekday.sun": "Neděle",

    "settingsGoals.nonWorking.title": "Nepracovní dny",
    "settingsGoals.nonWorking.add": "+ Přidat nepracovní den",
    "settingsGoals.nonWorking.empty":
      "Žádné nepracovní dny v rozsahu posledních 30 a příštích 90 dnů.",
    "settingsGoals.nonWorking.remove": "Odebrat {date}",
    "settingsGoals.nonWorking.pagination": "Stránkování nepracovních dnů",
    "settingsGoals.nonWorking.previous": "← Předchozí",
    "settingsGoals.nonWorking.next": "Další →",

    "settingsGoals.reason.vacation": "Dovolená",
    "settingsGoals.reason.holiday": "Svátek",
    "settingsGoals.reason.personal": "Osobní",

    "settingsGoals.dialog.title": "Přidat nepracovní den",
    "settingsGoals.dialog.date": "Datum",
    "settingsGoals.dialog.reason": "Důvod",
    "settingsGoals.dialog.description": "Popis (volitelné)",
    "settingsGoals.dialog.descriptionPlaceholder": "např. Velikonoční pondělí",
    "settingsGoals.dialog.cancel": "Zrušit",
    "settingsGoals.dialog.save": "Uložit",
  },
  en: {
    "settingsGoals.heading": "Goals",

    "settingsGoals.dailyGoal.title": "Daily hours goal",
    "settingsGoals.dailyGoal.description":
      "How many hours you aim to work each day. Used in the Goals view.",

    "settingsGoals.pomodoro.title": "Pomodoro",
    "settingsGoals.pomodoro.enable": "Enable Pomodoro",
    "settingsGoals.pomodoro.description":
      "While the timer is running, the app sends a notification after the work cycle completes and again after the break. The cycle is not saved anywhere — it only serves as a reminder.",
    "settingsGoals.pomodoro.work": "Work (min)",
    "settingsGoals.pomodoro.break": "Break (min)",

    "settingsGoals.workingWeek.title": "Working days of the week",
    "settingsGoals.workingWeek.description":
      "Which days of the week you usually work. Weekends and public holidays are not counted toward goals.",
    "settingsGoals.weekday.mon": "Monday",
    "settingsGoals.weekday.tue": "Tuesday",
    "settingsGoals.weekday.wed": "Wednesday",
    "settingsGoals.weekday.thu": "Thursday",
    "settingsGoals.weekday.fri": "Friday",
    "settingsGoals.weekday.sat": "Saturday",
    "settingsGoals.weekday.sun": "Sunday",

    "settingsGoals.nonWorking.title": "Non-working days",
    "settingsGoals.nonWorking.add": "+ Add non-working day",
    "settingsGoals.nonWorking.empty":
      "No non-working days within the last 30 and next 90 days.",
    "settingsGoals.nonWorking.remove": "Remove {date}",
    "settingsGoals.nonWorking.pagination": "Non-working days pagination",
    "settingsGoals.nonWorking.previous": "← Previous",
    "settingsGoals.nonWorking.next": "Next →",

    "settingsGoals.reason.vacation": "Vacation",
    "settingsGoals.reason.holiday": "Holiday",
    "settingsGoals.reason.personal": "Personal",

    "settingsGoals.dialog.title": "Add non-working day",
    "settingsGoals.dialog.date": "Date",
    "settingsGoals.dialog.reason": "Reason",
    "settingsGoals.dialog.description": "Description (optional)",
    "settingsGoals.dialog.descriptionPlaceholder": "e.g. Easter Monday",
    "settingsGoals.dialog.cancel": "Cancel",
    "settingsGoals.dialog.save": "Save",
  },
} as const;

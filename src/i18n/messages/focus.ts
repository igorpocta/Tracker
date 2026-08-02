/** Focus mode strings — settings panel, sidebar, popover, overlay. */
export const focus = {
  cs: {
    // Shared
    "focus.title": "Focus mode",
    "focus.start": "Spustit Focus",
    "focus.stop": "Zastavit Focus",
    "focus.running": "Focus běží",
    "focus.idle": "Focus je vypnutý",
    "focus.openEnded": "Bez časového omezení",
    "focus.remaining": "Zbývá {time}",
    "focus.toggleAria": "Spustit nebo zastavit Focus mode",

    // Overlay
    "focus.overlay.hidden": "{app} je během Focusu blokovaná.",
    "focus.overlay.killed": "{app} byla ukončena.",

    // Settings — intro
    "focus.settings.intro":
      "Focus mode schová rozptylující aplikace a přesměruje blokované weby na místní stránku. Není to zámek — Tracker můžete kdykoli vypnout.",
    "focus.settings.sessionTitle": "Relace",
    "focus.settings.durationLabel": "Výchozí délka",
    "focus.settings.durationOpen": "Bez omezení",
    "focus.settings.durationMinutes": "{count} min",

    // Settings — apps
    "focus.settings.appsTitle": "Aplikace",
    "focus.settings.appsIntro":
      "Tracker zasahuje jen do aplikace, kterou právě přepnete do popředí. Systémové aplikace jsou chráněné a zablokovat je nelze.",
    "focus.mode.label": "Režim",
    "focus.mode.blockSelected": "Blokovat vybrané",
    "focus.mode.allowOnly": "Povolit jen vybrané",
    "focus.mode.hiddenBlocked":
      "Blokovaná pravidla ({count}) zůstávají uložená, ale v tomto režimu se neuplatní — povolené má vždy přednost a vše ostatní je zakázané tak jako tak.",
    "focus.mode.app.blockHint":
      "Blokuje se jen to, co je v seznamu. Výjimky mají přednost — hodí se, když chcete zakázat aplikaci, ale jednu její část nechat.",
    "focus.mode.app.allowHint":
      "Vše kromě povolených se schová. V tomto režimu se nikdy nic neukončuje, jen skrývá. Dokud je seznam prázdný, režim se neuplatní.",
    "focus.settings.pickApp": "Vybrat ze spuštěných",
    "focus.settings.appPlaceholder": "com.slack.Slack nebo slack.exe",
    "focus.settings.protected": "chráněná",

    // Settings — sites
    "focus.settings.sitesTitle": "Weby",
    "focus.settings.sitesIntro":
      "Doména (seznam.cz) pokryje celý web včetně subdomén — email.seznam.cz i seznam.cz/email. Konkrétní host (www.seznam.cz) platí jen pro něj, email.seznam.cz projde. Lze zadat i cestu: reddit.com/r/rust.",
    "focus.mode.site.blockHint":
      "Blokují se jen weby ze seznamu. Výjimky mají přednost — třeba zakázat reddit.com, ale povolit reddit.com/r/rust.",
    "focus.mode.site.allowHint":
      "Vše kromě povolených skončí na blokovací stránce, kde se povolené weby zobrazí jako dlaždice. Dokud je seznam prázdný, režim se neuplatní.",
    "focus.settings.sitePlaceholder": "seznam.cz nebo www.seznam.cz",
    "focus.settings.extensionOk": "Rozšíření prohlížeče je připojené.",
    "focus.settings.extensionMissing":
      "Rozšíření prohlížeče se zatím neozvalo. Bez něj blokujeme weby jen v Safari a Chrome na macOS.",

    // Settings — notifications
    "focus.settings.notificationsTitle": "Notifikace",
    "focus.settings.blockNotifications": "Ztlumit notifikace během Focusu",
    "focus.settings.shortcutsIntro":
      "macOS neumožňuje zapnout režim Soustředění přímo. Vytvořte si ve Zkratkách dvě zkratky (zapnout a vypnout Soustředění) a vyberte je zde.",
    "focus.settings.shortcutOn": "Zkratka pro zapnutí",
    "focus.settings.shortcutOff": "Zkratka pro vypnutí",
    "focus.settings.shortcutNone": "Nepoužívat",
    "focus.settings.shortcutReload": "Načíst zkratky znovu",
    "focus.settings.windowsDnd":
      "Windows nemá pro Nerušit veřejné rozhraní. Otevřete systémové nastavení a přepněte ho ručně.",
    "focus.settings.openDnd": "Otevřít nastavení Nerušit",

    // Rules list
    "focus.rules.block": "Blokované",
    "focus.rules.allow": "Povolené",
    "focus.rules.exceptions": "Výjimky",
    "focus.rules.empty": "Zatím nic.",
    "focus.rules.add": "Přidat",
    "focus.rules.remove": "Odebrat",
    "focus.rules.enabled": "Zapnuto",
    "focus.rules.actionHide": "Schovat",
    "focus.rules.actionKill": "Ukončit",
    "focus.rules.actionHint":
      "Ukončení zavře aplikaci i s neuloženou prací. Používejte jen tam, kde o nic nepřijdete.",
  },
  en: {
    "focus.title": "Focus mode",
    "focus.start": "Start Focus",
    "focus.stop": "Stop Focus",
    "focus.running": "Focus is running",
    "focus.idle": "Focus is off",
    "focus.openEnded": "No time limit",
    "focus.remaining": "{time} left",
    "focus.toggleAria": "Start or stop Focus mode",

    "focus.overlay.hidden": "{app} is blocked during Focus.",
    "focus.overlay.killed": "{app} was closed.",

    "focus.settings.intro":
      "Focus mode hides distracting apps and redirects blocked sites to a local page. It is not a lock — you can always quit Tracker.",
    "focus.settings.sessionTitle": "Session",
    "focus.settings.durationLabel": "Default length",
    "focus.settings.durationOpen": "No limit",
    "focus.settings.durationMinutes": "{count} min",

    "focus.settings.appsTitle": "Applications",
    "focus.settings.appsIntro":
      "Tracker only touches the app you just brought to the front. System apps are protected and cannot be blocked.",
    "focus.mode.label": "Mode",
    "focus.mode.blockSelected": "Block selected",
    "focus.mode.allowOnly": "Allow only selected",
    "focus.mode.hiddenBlocked":
      "Block rules ({count}) are kept but do nothing in this mode — an allow rule always wins, and everything unlisted is blocked anyway.",
    "focus.mode.app.blockHint":
      "Only what is listed gets blocked. Exceptions win, which is how you block an app but keep one part of it.",
    "focus.mode.app.allowHint":
      "Everything but the allowed apps gets hidden. This mode never terminates anything, and stays inert while the list is empty.",
    "focus.settings.pickApp": "Pick from running apps",
    "focus.settings.appPlaceholder": "com.slack.Slack or slack.exe",
    "focus.settings.protected": "protected",

    "focus.settings.sitesTitle": "Websites",
    "focus.settings.sitesIntro":
      "A domain (seznam.cz) covers the whole site including subdomains — email.seznam.cz and seznam.cz/email alike. A specific host (www.seznam.cz) covers only that host, leaving email.seznam.cz reachable. A path works too: reddit.com/r/rust.",
    "focus.mode.site.blockHint":
      "Only listed sites get blocked. Exceptions win — block reddit.com but allow reddit.com/r/rust.",
    "focus.mode.site.allowHint":
      "Everything but the allowed sites lands on the block page, which lists them as tiles. Stays inert while the list is empty.",
    "focus.settings.sitePlaceholder": "seznam.cz or www.seznam.cz",
    "focus.settings.extensionOk": "The browser extension is connected.",
    "focus.settings.extensionMissing":
      "The browser extension hasn't checked in. Without it we can only block sites in Safari and Chrome on macOS.",

    "focus.settings.notificationsTitle": "Notifications",
    "focus.settings.blockNotifications": "Silence notifications during Focus",
    "focus.settings.shortcutsIntro":
      "macOS has no API for turning Focus on. Create two Shortcuts (turn Focus on and off) and select them here.",
    "focus.settings.shortcutOn": "Shortcut to turn on",
    "focus.settings.shortcutOff": "Shortcut to turn off",
    "focus.settings.shortcutNone": "Don't use",
    "focus.settings.shortcutReload": "Reload shortcuts",
    "focus.settings.windowsDnd":
      "Windows has no public API for Do Not Disturb. Open system settings and switch it manually.",
    "focus.settings.openDnd": "Open Do Not Disturb settings",

    "focus.rules.block": "Blocked",
    "focus.rules.allow": "Allowed",
    "focus.rules.exceptions": "Exceptions",
    "focus.rules.empty": "Nothing yet.",
    "focus.rules.add": "Add",
    "focus.rules.remove": "Remove",
    "focus.rules.enabled": "Enabled",
    "focus.rules.actionHide": "Hide",
    "focus.rules.actionKill": "Close",
    "focus.rules.actionHint":
      "Closing quits the app along with any unsaved work. Only use it where nothing is at stake.",
  },
} as const;

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
    "focus.settings.strictApps": "Povolit jen vybrané aplikace",
    "focus.settings.strictAppsHint":
      "Vše ostatní se schová. V tomto režimu se nikdy nic neukončuje, jen skrývá. Dokud je seznam povolených prázdný, režim se neuplatní.",
    "focus.settings.pickApp": "Vybrat ze spuštěných",
    "focus.settings.appPlaceholder": "com.slack.Slack nebo slack.exe",
    "focus.settings.protected": "chráněná",

    // Settings — sites
    "focus.settings.sitesTitle": "Weby",
    "focus.settings.sitesIntro":
      "Zadejte doménu (seznam.cz) nebo doménu s cestou (reddit.com/r/rust). Blokuje se přesně zadaný host — www.seznam.cz není totéž co seznam.cz. Pro celou doménu včetně subdomén použijte *.seznam.cz.",
    "focus.settings.strictSites": "Povolit jen vybrané weby",
    "focus.settings.strictSitesHint":
      "Vše ostatní skončí na blokovací stránce, kde se povolené weby zobrazí jako dlaždice. Dokud je seznam povolených prázdný, režim se neuplatní.",
    "focus.settings.sitePlaceholder": "seznam.cz nebo *.seznam.cz",
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
    "focus.settings.strictApps": "Allow only the selected apps",
    "focus.settings.strictAppsHint":
      "Everything else gets hidden. This mode never terminates anything, and stays inert while the allow list is empty.",
    "focus.settings.pickApp": "Pick from running apps",
    "focus.settings.appPlaceholder": "com.slack.Slack or slack.exe",
    "focus.settings.protected": "protected",

    "focus.settings.sitesTitle": "Websites",
    "focus.settings.sitesIntro":
      "Enter a domain (seznam.cz) or a domain with a path (reddit.com/r/rust). The host matches exactly — www.seznam.cz is not the same as seznam.cz. Use *.seznam.cz to cover a domain and all its subdomains.",
    "focus.settings.strictSites": "Allow only the selected sites",
    "focus.settings.strictSitesHint":
      "Everything else lands on the block page, which lists the allowed sites as tiles. Stays inert while the allow list is empty.",
    "focus.settings.sitePlaceholder": "seznam.cz or *.seznam.cz",
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

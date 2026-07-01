/** Reports route — period summary, daily bar chart, issues breakdown, export. */
export const reports = {
  cs: {
    "reports.heading": "Reporty",
    "reports.period.aria": "Období",

    "reports.period.this-week": "Tento týden",
    "reports.period.last-week": "Minulý týden",
    "reports.period.this-month": "Tento měsíc",
    "reports.period.last-month": "Minulý měsíc",
    "reports.period.last-30": "Posledních 30 dní",
    "reports.period.this-year": "Od začátku roku",

    "reports.streak.day": "den",
    "reports.streak.days2to4": "dny",
    "reports.streak.daysMany": "dní",
    "reports.streak.tooltipRecord":
      "Po sobě jdoucí pracovní dny se splněným denním cílem · osobní rekord!",
    "reports.streak.tooltip":
      "Po sobě jdoucí pracovní dny se splněným denním cílem · nejdelší {longest}",
    "reports.streak.record": "· rekord {longest}",
    "reports.streak.todayPending": "· dnes ještě",

    "reports.breakdown.heading": "Rozpad úkolů",
    "reports.breakdown.issue": "Úkol",
    "reports.breakdown.description": "Popis",
    "reports.breakdown.total": "Celkem",
    "reports.breakdown.lastLogged": "Naposledy zaznamenáno",
    "reports.breakdown.empty": "Zatím prázdné.",
    "reports.breakdown.loadingSummary": "(načítá se…)",

    "reports.chart.heading": "Hodiny za den",
    "reports.chart.goalAria": "Denní cíl {hours} h",
    "reports.chart.goalLabel": "cíl {hours}h",
    "reports.chart.tooltipTotal": "{duration} celkem",

    "reports.export.title": "Exportovat výkaz do Excelu (.xlsx)",
    "reports.export.button": "Exportovat do Excelu",
  },
  en: {
    "reports.heading": "Reports",
    "reports.period.aria": "Period",

    "reports.period.this-week": "This week",
    "reports.period.last-week": "Last week",
    "reports.period.this-month": "This month",
    "reports.period.last-month": "Last month",
    "reports.period.last-30": "Last 30 days",
    "reports.period.this-year": "Year to date",

    "reports.streak.day": "day",
    "reports.streak.days2to4": "days",
    "reports.streak.daysMany": "days",
    "reports.streak.tooltipRecord":
      "Consecutive working days meeting the daily goal · personal record!",
    "reports.streak.tooltip":
      "Consecutive working days meeting the daily goal · longest {longest}",
    "reports.streak.record": "· record {longest}",
    "reports.streak.todayPending": "· not yet today",

    "reports.breakdown.heading": "Issues breakdown",
    "reports.breakdown.issue": "Issue",
    "reports.breakdown.description": "Description",
    "reports.breakdown.total": "Total",
    "reports.breakdown.lastLogged": "Last logged",
    "reports.breakdown.empty": "Nothing yet.",
    "reports.breakdown.loadingSummary": "(loading…)",

    "reports.chart.heading": "Daily hours",
    "reports.chart.goalAria": "Daily goal {hours} h",
    "reports.chart.goalLabel": "goal {hours}h",
    "reports.chart.tooltipTotal": "{duration} total",

    "reports.export.title": "Export the report to Excel (.xlsx)",
    "reports.export.button": "Export to Excel",
  },
} as const;

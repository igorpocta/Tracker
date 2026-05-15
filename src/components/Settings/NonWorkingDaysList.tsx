/**
 * Settings → Cíle — list of upcoming/recent non-working days.
 *
 * Shows `listNonWorkingDays(today - 30, today + 90)` so the user sees what
 * is on the immediate horizon without scrolling forever. Each row is
 * removable; an "Add" button at the bottom opens `AddNonWorkingDayDialog`.
 *
 *   Datum         Den   Důvod                       Popis           ×
 *   15.05.2026    Pá    🏖 Dovolená                 Bali             ×
 *   23.05.2026    So    🎉 Svátek                   ...              ×
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import {
  addNonWorkingDay,
  listNonWorkingDays,
  removeNonWorkingDay,
} from "../../api/commands";
import type { NonWorkingDay } from "../../api/commands";
import { addDays, formatIsoDate } from "../../lib/dates";

import { AddNonWorkingDayDialog } from "./AddNonWorkingDayDialog";

const WEEKDAYS_SHORT = ["Ne", "Po", "Út", "St", "Čt", "Pá", "So"];

function reasonMeta(reason: string): { icon: string; label: string } {
  switch (reason) {
    case "vacation":
      return { icon: "🏖", label: "Dovolená" };
    case "holiday":
      return { icon: "🎉", label: "Svátek" };
    case "personal":
      return { icon: "🙅", label: "Osobní" };
    default:
      return { icon: "•", label: reason };
  }
}

function formatDmy(iso: string): string {
  // Input is `YYYY-MM-DD` (backend-stable). Parsing via Date then re-
  // formatting keeps the locale-style DD.MM.YYYY consistent regardless of
  // browser locale.
  const [y, m, d] = iso.split("-").map((s) => parseInt(s, 10));
  return `${`${d}`.padStart(2, "0")}.${`${m}`.padStart(2, "0")}.${y}`;
}

function weekdayFor(iso: string): string {
  const [y, m, d] = iso.split("-").map((s) => parseInt(s, 10));
  const dt = new Date(y, m - 1, d);
  return WEEKDAYS_SHORT[dt.getDay()];
}

export function NonWorkingDaysList() {
  const queryClient = useQueryClient();
  const today = useMemo(() => new Date(), []);

  const fromIso = formatIsoDate(addDays(today, -30));
  const toIso = formatIsoDate(addDays(today, 90));

  const q = useQuery({
    queryKey: ["non-working-days", fromIso, toIso],
    queryFn: () => listNonWorkingDays(fromIso, toIso),
    staleTime: 60_000,
  });

  const [dialogOpen, setDialogOpen] = useState(false);

  const handleRemove = async (date: string) => {
    try {
      await removeNonWorkingDay(date);
      queryClient.invalidateQueries({ queryKey: ["non-working-days"] });
    } catch {
      /* swallow */
    }
  };

  const handleAdd = async (date: string, reason: string, label?: string) => {
    await addNonWorkingDay(date, reason, label);
    queryClient.invalidateQueries({ queryKey: ["non-working-days"] });
  };

  const days: NonWorkingDay[] = q.data ?? [];

  return (
    <section>
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm font-semibold text-[var(--text-primary)]">
          Nepracovní dny
        </span>
        <button
          type="button"
          onClick={() => setDialogOpen(true)}
          className="text-xs h-7 px-2 rounded-[var(--radius-md)]
                     text-[var(--accent)] hover:bg-[var(--accent-soft)]
                     transition-colors duration-150"
        >
          + Přidat nepracovní den
        </button>
      </div>

      {days.length === 0 ? (
        <p className="text-[12px] text-[var(--text-tertiary)]">
          Žádné nepracovní dny v rozsahu posledních 30 a příštích 90 dnů.
        </p>
      ) : (
        <ul className="flex flex-col gap-1">
          {days.map((d) => {
            const meta = reasonMeta(d.reason);
            return (
              <li
                key={d.date}
                className="grid grid-cols-[90px_36px_1fr_28px] items-center gap-2 px-2 h-8 rounded-[var(--radius-md)]
                           hover:bg-[var(--bg-hover)] transition-colors duration-150"
              >
                <span className="text-xs tabular-nums text-[var(--text-primary)]">
                  {formatDmy(d.date)}
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)]">
                  {weekdayFor(d.date)}
                </span>
                <span className="text-xs text-[var(--text-secondary)] truncate">
                  <span className="mr-2" aria-hidden>
                    {meta.icon}
                  </span>
                  {meta.label}
                  {d.label ? (
                    <span className="ml-2 text-[var(--text-tertiary)]">
                      — {d.label}
                    </span>
                  ) : null}
                </span>
                <button
                  type="button"
                  onClick={() => void handleRemove(d.date)}
                  aria-label={`Odebrat ${formatDmy(d.date)}`}
                  className="h-6 w-6 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]
                             rounded-[var(--radius-sm)] hover:bg-[var(--bg-hover)]
                             transition-colors duration-150"
                >
                  ×
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <AddNonWorkingDayDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        onSave={handleAdd}
      />
    </section>
  );
}

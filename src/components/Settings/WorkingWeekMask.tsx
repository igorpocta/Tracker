/**
 * Settings → Cíle — working-week mask editor.
 *
 *   Po ☑   Út ☑   St ☑   Čt ☑   Pá ☑   So ☐   Ne ☐
 *
 * Each checkbox toggles a single bit in the 7-bit mask:
 *
 *   Po=1  Út=2  St=4  Čt=8  Pá=16  So=32  Ne=64
 *
 * Default 31 (Mon–Fri). The bitmask layout matches the Rust backend so a
 * single integer round-trips between `get_working_week_mask` and
 * `set_working_week_mask`.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
  getWorkingWeekMask,
  setWorkingWeekMask,
} from "../../api/commands";
import { queryKeys } from "../../api/queryKeys";
import { useT } from "../../i18n";
import { formatWeekdayShort } from "../../lib/format";

// `dow` = JS getDay() index (Sun=0 … Sat=6) so the short label follows the UI
// language via `formatWeekdayShort`.
const DAYS: { bit: number; dow: number; labelKey: string }[] = [
  { bit: 1, dow: 1, labelKey: "settingsGoals.weekday.mon" },
  { bit: 2, dow: 2, labelKey: "settingsGoals.weekday.tue" },
  { bit: 4, dow: 3, labelKey: "settingsGoals.weekday.wed" },
  { bit: 8, dow: 4, labelKey: "settingsGoals.weekday.thu" },
  { bit: 16, dow: 5, labelKey: "settingsGoals.weekday.fri" },
  { bit: 32, dow: 6, labelKey: "settingsGoals.weekday.sat" },
  { bit: 64, dow: 0, labelKey: "settingsGoals.weekday.sun" },
];

const DEFAULT_MASK = 31; // Mon–Fri.

/**
 * Compute the new mask after toggling a single bit.
 *
 * Exposed for unit tests: changing the mask is pure integer arithmetic so we
 * can verify it without rendering anything.
 */
export function toggleBit(mask: number, bit: number): number {
  return (mask & bit) !== 0 ? mask & ~bit : mask | bit;
}

export function WorkingWeekMask() {
  const t = useT();
  const queryClient = useQueryClient();
  const q = useQuery({
    queryKey: queryKeys.workingWeekMask.all(),
    queryFn: getWorkingWeekMask,
    staleTime: 60_000,
  });

  /**
   * Optimistic local mirror — the input flips instantly on click and we
   * reconcile if the backend write fails. Without this the checkbox would
   * stay in its old position until React Query re-fetches.
   */
  const [localMask, setLocalMask] = useState<number | null>(null);
  useEffect(() => {
    if (q.data !== undefined) setLocalMask(q.data);
  }, [q.data]);

  const mask = localMask ?? q.data ?? DEFAULT_MASK;

  const handleToggle = async (bit: number) => {
    const next = toggleBit(mask, bit);
    setLocalMask(next);
    try {
      await setWorkingWeekMask(next);
      queryClient.invalidateQueries({ queryKey: queryKeys.workingWeekMask.all() });
      // Mask change affects every cached non-working derivation so the
      // calendar grid re-paints.
      queryClient.invalidateQueries({ queryKey: queryKeys.nonWorkingDays.all() });
    } catch {
      // Roll back on failure — the user shouldn't see a "stuck" checkbox.
      setLocalMask(mask);
    }
  };

  return (
    <section>
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-semibold text-[var(--text-primary)]">
          {t("settingsGoals.workingWeek.title")}
        </span>
      </div>
      <div
        role="group"
        aria-label={t("settingsGoals.workingWeek.title")}
        className="flex flex-wrap gap-2"
      >
        {DAYS.map((d) => {
          const active = (mask & d.bit) !== 0;
          return (
            <label
              key={d.bit}
              className="inline-flex items-center gap-2 px-3 h-8 rounded-[var(--radius-md)]
                         border text-sm cursor-pointer select-none transition-colors duration-150"
              style={{
                borderColor: active
                  ? "var(--accent)"
                  : "var(--border-subtle)",
                background: active
                  ? "var(--accent-soft)"
                  : "transparent",
                color: active
                  ? "var(--accent)"
                  : "var(--text-secondary)",
              }}
            >
              <input
                type="checkbox"
                checked={active}
                onChange={() => void handleToggle(d.bit)}
                aria-label={t(d.labelKey)}
                className="sr-only"
              />
              <span>{formatWeekdayShort(d.dow)}</span>
            </label>
          );
        })}
      </div>
      <p className="text-[11px] text-[var(--text-tertiary)] mt-3">
        {t("settingsGoals.workingWeek.description")}
      </p>
    </section>
  );
}

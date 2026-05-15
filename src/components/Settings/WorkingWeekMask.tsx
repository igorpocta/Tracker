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

const DAYS: { bit: number; short: string; label: string }[] = [
  { bit: 1, short: "Po", label: "Pondělí" },
  { bit: 2, short: "Út", label: "Úterý" },
  { bit: 4, short: "St", label: "Středa" },
  { bit: 8, short: "Čt", label: "Čtvrtek" },
  { bit: 16, short: "Pá", label: "Pátek" },
  { bit: 32, short: "So", label: "Sobota" },
  { bit: 64, short: "Ne", label: "Neděle" },
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
  const queryClient = useQueryClient();
  const q = useQuery({
    queryKey: ["working-week-mask"],
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
      queryClient.invalidateQueries({ queryKey: ["working-week-mask"] });
      // Mask change affects every cached non-working derivation so the
      // calendar grid re-paints.
      queryClient.invalidateQueries({ queryKey: ["non-working-days"] });
    } catch {
      // Roll back on failure — the user shouldn't see a "stuck" checkbox.
      setLocalMask(mask);
    }
  };

  return (
    <section>
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-semibold text-[var(--text-primary)]">
          Pracovní dny v týdnu
        </span>
      </div>
      <div
        role="group"
        aria-label="Pracovní dny v týdnu"
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
                aria-label={d.label}
                className="sr-only"
              />
              <span>{d.short}</span>
            </label>
          );
        })}
      </div>
      <p className="text-[11px] text-[var(--text-tertiary)] mt-3">
        Které dny v týdnu obvykle pracujete. Víkendy a státní svátky se
        nezapočítávají do cílů.
      </p>
    </section>
  );
}

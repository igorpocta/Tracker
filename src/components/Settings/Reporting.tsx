/**
 * Settings → Reporting.
 *
 * Reference: `screens/SCR-20260514-rjiy-2.png`.
 *
 *   Hourly rate     [ 2000        ]
 *   Currency        [ CZK — Czech koruna ▾ ]
 *
 * "Leave empty to disable the earnings card entirely." → an empty / 0 rate
 * hides the card on the Reports page.
 */
import { useEffect, useState } from "react";

import type { Currency } from "../../api/types";
import { usePrefsStore } from "../../stores/prefsStore";

const CURRENCIES: { value: Currency; label: string }[] = [
  { value: "CZK", label: "CZK — Czech koruna" },
  { value: "EUR", label: "EUR — Euro" },
  { value: "USD", label: "USD — US dollar" },
  { value: "GBP", label: "GBP — British pound" },
  { value: "PLN", label: "PLN — Polish złoty" },
  { value: "CHF", label: "CHF — Swiss franc" },
];

export default function Reporting() {
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);
  const setHourlyRate = usePrefsStore((s) => s.setHourlyRate);
  const setCurrency = usePrefsStore((s) => s.setCurrency);

  const [rateStr, setRateStr] = useState(`${hourlyRate || ""}`);

  useEffect(() => {
    setRateStr(`${hourlyRate || ""}`);
  }, [hourlyRate]);

  const handleRateBlur = async () => {
    const trimmed = rateStr.trim().replace(",", ".");
    const n = trimmed === "" ? 0 : Number(trimmed);
    if (!Number.isFinite(n) || n < 0) {
      setRateStr(`${hourlyRate || ""}`);
      return;
    }
    if (n !== hourlyRate) {
      await setHourlyRate(n);
    }
  };

  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Reporting
        </h2>
        <p className="text-xs text-[var(--text-tertiary)] mt-1 max-w-md">
          Set your hourly rate to see total earnings on the Reports tab.
          Earnings stay hidden behind a click-to-reveal eye icon — useful when
          working in openspaces.
        </p>
      </header>

      <section>
        <label className="block text-sm font-semibold text-[var(--text-primary)] mb-2">
          Hourly rate
        </label>
        <input
          type="text"
          inputMode="decimal"
          value={rateStr}
          onChange={(e) => setRateStr(e.target.value)}
          onBlur={handleRateBlur}
          placeholder="0"
          className="w-full h-9 px-3 rounded-[var(--radius-md)]
                     bg-transparent border border-[var(--border-subtle)] text-sm
                     text-[var(--text-primary)] focus:outline-none
                     focus:border-[var(--border-default)] transition-colors duration-150"
        />
        <p className="text-[11px] text-[var(--text-tertiary)] mt-2">
          Leave empty to disable the earnings card entirely.
        </p>
      </section>

      <section>
        <label className="block text-sm font-semibold text-[var(--text-primary)] mb-2">
          Currency
        </label>
        <select
          value={currency}
          onChange={(e) => void setCurrency(e.target.value as Currency)}
          className="w-full h-9 px-3 rounded-[var(--radius-md)]
                     bg-transparent border border-[var(--border-subtle)] text-sm
                     text-[var(--text-primary)] focus:outline-none
                     focus:border-[var(--border-default)] transition-colors duration-150"
        >
          {CURRENCIES.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      </section>
    </div>
  );
}

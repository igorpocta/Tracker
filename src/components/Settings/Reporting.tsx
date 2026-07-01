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
import {
  firstError,
  hourlyRateSchema,
  parseRateInput,
} from "../../lib/validation";
import { usePrefsStore } from "../../stores/prefsStore";
import { SettingsCard } from "./SettingsCard";

const CURRENCIES: { value: Currency; label: string }[] = [
  { value: "CZK", label: "CZK — Česká koruna" },
  { value: "EUR", label: "EUR — Euro" },
  { value: "USD", label: "USD — Americký dolar" },
  { value: "GBP", label: "GBP — Britská libra" },
  { value: "PLN", label: "PLN — Polský zlotý" },
  { value: "CHF", label: "CHF — Švýcarský frank" },
];

export default function Reporting() {
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);
  const setHourlyRate = usePrefsStore((s) => s.setHourlyRate);
  const setCurrency = usePrefsStore((s) => s.setCurrency);

  const [rateStr, setRateStr] = useState(`${hourlyRate || ""}`);
  const [rateError, setRateError] = useState<string | null>(null);

  useEffect(() => {
    setRateStr(`${hourlyRate || ""}`);
    setRateError(null);
  }, [hourlyRate]);

  const handleRateChange = (next: string) => {
    setRateStr(next);
    // Live-validate: surface the error inline as the user types so they can
    // see why the value will be rejected on blur. We don't update the store
    // until blur to avoid spamming the backend.
    const n = parseRateInput(next);
    if (n === null) {
      setRateError("musí být platné číslo (např. 1500)");
    } else {
      setRateError(firstError(hourlyRateSchema, n));
    }
  };

  const handleRateBlur = async () => {
    const n = parseRateInput(rateStr);
    if (n === null) {
      // Parse failed (scientific notation, junk chars, …) — revert.
      setRateStr(`${hourlyRate || ""}`);
      setRateError(null);
      return;
    }
    const err = firstError(hourlyRateSchema, n);
    if (err) {
      setRateError(err);
      return;
    }
    setRateError(null);
    if (n !== hourlyRate) {
      await setHourlyRate(n);
    }
  };

  return (
    <div className="flex flex-col gap-5 w-full max-w-3xl">
      <header>
        <h2 className="text-lg font-semibold text-[var(--text-primary)]">
          Reporting
        </h2>
        <p className="text-xs text-[var(--text-tertiary)] mt-1 max-w-md">
          Nastavte hodinovou sazbu a uvidíte celkový výdělek v sekci Reporty.
          Výdělek zůstává skrytý za kliknutím na ikonu oka — vhodné při práci
          v open space.
        </p>
      </header>

      <SettingsCard
        title="Hodinová sazba"
        description="Ponechte prázdné a karta výdělků se úplně skryje."
      >
        <input
          type="text"
          inputMode="decimal"
          aria-label="Hodinová sazba"
          value={rateStr}
          onChange={(e) => handleRateChange(e.target.value)}
          onBlur={handleRateBlur}
          placeholder="0"
          aria-invalid={rateError != null}
          aria-describedby={rateError ? "rate-error" : undefined}
          className="ui-input w-full aria-[invalid=true]:border-[var(--danger,#dc2626)]"
        />
        {rateError && (
          <p id="rate-error" className="text-[11px] text-[var(--danger,#dc2626)] mt-2">
            {rateError}
          </p>
        )}
      </SettingsCard>

      <SettingsCard title="Měna">
        <select
          value={currency}
          onChange={(e) => void setCurrency(e.target.value as Currency)}
          aria-label="Měna"
          className="ui-select w-full"
        >
          {CURRENCIES.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      </SettingsCard>
    </div>
  );
}


/**
 * Settings → Time tab.
 *
 * Three controls:
 *   - Daily goal (hours, number input).
 *   - Hourly rate + currency dropdown.
 *   - Widget format (display format for the tray timer).
 */
import { useEffect, useState } from "react";

import type { Currency } from "../../api/types";
import { Button } from "../../components/common/Button";
import { Select } from "../../components/common/Select";
import { Spinner } from "../../components/common/Spinner";
import { usePrefsStore, type WidgetFormat } from "../../stores/prefsStore";

const CURRENCY_OPTIONS = [
  { value: "CZK", label: "CZK · Czech koruna" },
  { value: "EUR", label: "EUR · Euro" },
  { value: "USD", label: "USD · US dollar" },
  { value: "GBP", label: "GBP · British pound" },
  { value: "PLN", label: "PLN · Polish złoty" },
  { value: "CHF", label: "CHF · Swiss franc" },
];

export default function TimeSettings() {
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const currency = usePrefsStore((s) => s.currency);
  const widgetFormat = usePrefsStore((s) => s.widgetFormat);

  const setDailyGoal = usePrefsStore((s) => s.setDailyGoal);
  const setHourlyRate = usePrefsStore((s) => s.setHourlyRate);
  const setCurrency = usePrefsStore((s) => s.setCurrency);
  const setWidgetFormat = usePrefsStore((s) => s.setWidgetFormat);

  const [hoursStr, setHoursStr] = useState(`${dailyGoalSeconds / 3600}`);
  const [rateStr, setRateStr] = useState(`${hourlyRate}`);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setHoursStr(`${dailyGoalSeconds / 3600}`);
  }, [dailyGoalSeconds]);
  useEffect(() => {
    setRateStr(`${hourlyRate}`);
  }, [hourlyRate]);

  const handleSave = async () => {
    setError(null);
    setSaved(false);
    const hoursNum = Number(hoursStr.replace(",", "."));
    const rateNum = Number(rateStr.replace(",", "."));
    if (!Number.isFinite(hoursNum) || hoursNum < 0) {
      setError("Daily goal must be a non-negative number of hours.");
      return;
    }
    if (!Number.isFinite(rateNum) || rateNum < 0) {
      setError("Hourly rate must be a non-negative number.");
      return;
    }
    setSaving(true);
    try {
      await Promise.all([
        setDailyGoal(Math.round(hoursNum * 3600)),
        setHourlyRate(rateNum),
      ]);
      setSaved(true);
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to save preferences.");
    } finally {
      setSaving(false);
    }
  };

  const inputCls =
    "px-2.5 h-8 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)] " +
    "focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)] " +
    "text-sm text-[var(--text-primary)] transition-colors duration-150";

  return (
    <div className="flex flex-col gap-6 max-w-xl">
      <Section title="Daily goal" description="Number of hours you aim to log per day.">
        <div className="flex items-center gap-2">
          <input
            type="text"
            inputMode="decimal"
            value={hoursStr}
            onChange={(e) => setHoursStr(e.target.value)}
            className={`${inputCls} w-24`}
            aria-label="Daily goal hours"
          />
          <span className="text-xs text-[var(--text-tertiary)]">hours</span>
        </div>
      </Section>

      <Section
        title="Hourly rate"
        description="Used for earnings on the Today and Reports views. Set to 0 to hide."
      >
        <div className="flex items-center gap-2 flex-wrap">
          <input
            type="text"
            inputMode="decimal"
            value={rateStr}
            onChange={(e) => setRateStr(e.target.value)}
            className={`${inputCls} w-32`}
            aria-label="Hourly rate"
          />
          <Select
            value={currency}
            onChange={(e) => void setCurrency(e.target.value as Currency)}
            options={CURRENCY_OPTIONS}
            aria-label="Currency"
          />
          <span className="text-xs text-[var(--text-tertiary)]">/ hour</span>
        </div>
      </Section>

      <Section title="Widget format" description="How the tray icon displays elapsed time.">
        <Select
          value={widgetFormat}
          onChange={(e) => void setWidgetFormat(e.target.value as WidgetFormat)}
          options={[
            { value: "HH:MM:SS", label: "HH:MM:SS  (01:23:45)" },
            { value: "Hh Mm", label: "Hh Mm  (1h 23m)" },
            { value: "0.0h", label: "0.0h  (1.4h)" },
          ]}
          aria-label="Widget format"
        />
      </Section>

      <div className="pt-2 flex items-center gap-3">
        <Button variant="primary" size="md" onClick={handleSave} disabled={saving}>
          {saving && <Spinner className="w-3.5 h-3.5" />}
          Save changes
        </Button>
        {saved && (
          <span className="text-xs text-[var(--success)]">Saved.</span>
        )}
        {error && (
          <span className="text-xs text-[var(--danger)]" role="alert">
            {error}
          </span>
        )}
      </div>
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">{title}</h3>
        {description && (
          <p className="text-xs text-[var(--text-tertiary)] mt-0.5">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

/**
 * Small popover for editing the daily goal + hourly rate. Triggered from
 * the goal bar's pencil icon.
 *
 * Both values write through `prefsStore` actions, which in turn invoke the
 * relevant Tauri commands. We intentionally render an inline panel rather
 * than a floating popover (no headless-ui dep): it sits above the goal bar
 * in the right-panel.
 */
import { useEffect, useState } from "react";

import { usePrefsStore } from "../../stores/prefsStore";
import { Button } from "../common/Button";

export interface GoalSettingsProps {
  open: boolean;
  onClose: () => void;
}

export function GoalSettings({ open, onClose }: GoalSettingsProps) {
  const dailyGoalSeconds = usePrefsStore((s) => s.dailyGoalSeconds);
  const hourlyRate = usePrefsStore((s) => s.hourlyRate);
  const setDailyGoal = usePrefsStore((s) => s.setDailyGoal);
  const setHourlyRate = usePrefsStore((s) => s.setHourlyRate);

  const [hours, setHours] = useState<string>(
    (dailyGoalSeconds / 3600).toString(),
  );
  const [rate, setRate] = useState<string>(hourlyRate.toString());
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Re-seed on open so the form reflects the current store values.
  useEffect(() => {
    if (open) {
      setHours((dailyGoalSeconds / 3600).toString());
      setRate(hourlyRate.toString());
      setError(null);
    }
  }, [open, dailyGoalSeconds, hourlyRate]);

  if (!open) return null;

  const handleSave = async () => {
    const hoursNum = Number(hours.replace(",", "."));
    const rateNum = Number(rate.replace(",", "."));
    if (!Number.isFinite(hoursNum) || hoursNum < 0) {
      setError("Daily hours must be a non-negative number.");
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
      onClose();
    } catch (e) {
      setError(typeof e === "string" ? e : "Failed to save preferences.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="border border-[var(--border-subtle)] bg-[var(--bg-elevated)] rounded-[var(--radius-md)] p-3 flex flex-col gap-3"
      role="dialog"
      aria-label="Daily goal settings"
    >
      <div className="flex flex-col gap-1.5">
        <label htmlFor="goal-hours" className="text-xs font-medium text-[var(--text-secondary)]">
          Daily goal (hours)
        </label>
        <input
          id="goal-hours"
          type="text"
          inputMode="decimal"
          value={hours}
          onChange={(e) => setHours(e.target.value)}
          autoFocus
          className="px-2.5 h-8 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)]
                     focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                     text-sm text-[var(--text-primary)] w-24 transition-colors duration-150"
        />
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor="goal-rate" className="text-xs font-medium text-[var(--text-secondary)]">
          Hourly rate
          <span className="text-[var(--text-tertiary)] font-normal ml-1.5">
            (0 to hide)
          </span>
        </label>
        <input
          id="goal-rate"
          type="text"
          inputMode="decimal"
          value={rate}
          onChange={(e) => setRate(e.target.value)}
          className="px-2.5 h-8 rounded-[var(--radius-md)] bg-transparent border border-[var(--border-default)]
                     focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]
                     text-sm text-[var(--text-primary)] w-32 transition-colors duration-150"
        />
      </div>

      {error && (
        <p role="alert" className="text-xs text-[var(--danger)]">
          {error}
        </p>
      )}

      <div className="flex items-center justify-end gap-1.5">
        <Button variant="secondary" size="sm" onClick={onClose} disabled={saving}>
          Cancel
        </Button>
        <Button variant="primary" size="sm" onClick={handleSave} disabled={saving}>
          Save
        </Button>
      </div>
    </div>
  );
}

/**
 * Range picker for the Reports route.
 *
 * Five presets + a custom mode that shows two `<input type="date">` fields.
 */
import { useId } from "react";

import { RadioGroup } from "../common/Radio";
import { formatIsoDate } from "../../lib/dates";
import type { RangePreset } from "../../hooks/useDateRange";

export interface RangePickerProps {
  preset: RangePreset;
  from: Date;
  to: Date;
  onPresetChange: (next: RangePreset) => void;
  onFromChange: (date: Date) => void;
  onToChange: (date: Date) => void;
}

const OPTIONS: { value: RangePreset; label: string }[] = [
  { value: "last_7", label: "Last 7 days" },
  { value: "last_30", label: "Last 30 days" },
  { value: "this_month", label: "This month" },
  { value: "last_month", label: "Last month" },
  { value: "custom", label: "Custom" },
];

export function RangePicker({
  preset,
  from,
  to,
  onPresetChange,
  onFromChange,
  onToChange,
}: RangePickerProps) {
  const fromId = useId();
  const toId = useId();
  return (
    <div className="flex flex-wrap items-center gap-3">
      <RadioGroup<RangePreset>
        label="Date range"
        options={OPTIONS}
        value={preset}
        onChange={onPresetChange}
      />
      {preset === "custom" && (
        <div className="inline-flex items-center gap-2">
          <label htmlFor={fromId} className="text-xs text-[var(--text-tertiary)]">
            From
          </label>
          <input
            id={fromId}
            type="date"
            value={formatIsoDate(from)}
            onChange={(e) => {
              const v = e.target.value;
              if (!v) return;
              const [y, m, d] = v.split("-").map(Number);
              onFromChange(new Date(y, m - 1, d));
            }}
            className="bg-transparent border border-[var(--border-default)] rounded-[var(--radius-md)] h-8 px-2 text-xs text-[var(--text-primary)]
                       focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
          />
          <label htmlFor={toId} className="text-xs text-[var(--text-tertiary)]">
            To
          </label>
          <input
            id={toId}
            type="date"
            value={formatIsoDate(to)}
            onChange={(e) => {
              const v = e.target.value;
              if (!v) return;
              const [y, m, d] = v.split("-").map(Number);
              onToChange(new Date(y, m - 1, d));
            }}
            className="bg-transparent border border-[var(--border-default)] rounded-[var(--radius-md)] h-8 px-2 text-xs text-[var(--text-primary)]
                       focus:border-[var(--accent)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
          />
        </div>
      )}
    </div>
  );
}

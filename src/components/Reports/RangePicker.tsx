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
          <label htmlFor={fromId} className="text-xs text-neutral-400">
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
            className="bg-neutral-950 border border-neutral-800 rounded-md px-2 py-1 text-xs"
          />
          <label htmlFor={toId} className="text-xs text-neutral-400">
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
            className="bg-neutral-950 border border-neutral-800 rounded-md px-2 py-1 text-xs"
          />
        </div>
      )}
    </div>
  );
}

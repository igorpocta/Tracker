/**
 * Summary cards row at the top of the Reports page: total hours, daily
 * average, days worked.
 */
import type { ReactNode } from "react";

import { Card } from "../common/Card";

export interface SummaryCardsProps {
  totalSeconds: number;
  daysInRange: number;
  daysWorked: number;
}

export function SummaryCards({
  totalSeconds,
  daysInRange,
  daysWorked,
}: SummaryCardsProps) {
  const hours = totalSeconds / 3600;
  const avg = daysInRange > 0 ? hours / daysInRange : 0;
  const avgWorked = daysWorked > 0 ? hours / daysWorked : 0;
  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
      <StatCard label="Total" value={`${formatHoursFixed(hours)}h`} hint={`${daysInRange} days in range`} />
      <StatCard
        label="Avg / day (range)"
        value={`${formatHoursFixed(avg)}h`}
        hint="across every day"
      />
      <StatCard
        label="Avg / day worked"
        value={`${formatHoursFixed(avgWorked)}h`}
        hint={`${daysWorked} ${daysWorked === 1 ? "day" : "days"} with logs`}
      />
    </div>
  );
}

function StatCard({
  label,
  value,
  hint,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
}) {
  return (
    <Card padding="md">
      <div className="text-[11px] uppercase tracking-wider text-neutral-500">
        {label}
      </div>
      <div className="text-2xl font-mono tabular-nums mt-1 text-neutral-50">
        {value}
      </div>
      {hint && <div className="text-[11px] text-neutral-500 mt-0.5">{hint}</div>}
    </Card>
  );
}

function formatHoursFixed(hours: number): string {
  if (!Number.isFinite(hours) || hours <= 0) return "0";
  const rounded = Math.round(hours * 10) / 10;
  return Number.isInteger(rounded) ? `${rounded}` : rounded.toFixed(1);
}

/**
 * Summary cards row at the top of the Reports page.
 *
 * When `hourlyRate > 0`, an Earnings card is rendered first and given visual
 * primacy (accent value, slightly larger weight). Otherwise we show the
 * standard three-tile layout: total / avg per range day / avg per worked day.
 */
import type { ReactNode } from "react";

import { formatMoney } from "../../lib/format";
import { Card } from "../common/Card";

export interface SummaryCardsProps {
  totalSeconds: number;
  daysInRange: number;
  daysWorked: number;
  hourlyRate: number;
  currency: string;
}

export function SummaryCards({
  totalSeconds,
  daysInRange,
  daysWorked,
  hourlyRate,
  currency,
}: SummaryCardsProps) {
  const hours = totalSeconds / 3600;
  const avg = daysInRange > 0 ? hours / daysInRange : 0;
  const avgWorked = daysWorked > 0 ? hours / daysWorked : 0;
  const earnings = hourlyRate > 0 ? hours * hourlyRate : 0;

  if (hourlyRate > 0) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
        <StatCard
          accent
          label="Earnings"
          value={formatMoney(earnings, currency)}
          hint={`${formatMoney(hourlyRate, currency)}/h`}
        />
        <StatCard
          label="Total"
          value={`${formatHoursFixed(hours)}h`}
          hint={`${daysInRange} days in range`}
        />
        <StatCard
          label="Avg / day"
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

  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
      <StatCard
        label="Total"
        value={`${formatHoursFixed(hours)}h`}
        hint={`${daysInRange} days in range`}
      />
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
  accent = false,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  accent?: boolean;
}) {
  return (
    <Card padding="md">
      <div className="text-[10px] uppercase tracking-[0.12em] text-[var(--text-tertiary)]">
        {label}
      </div>
      <div
        className={
          accent
            ? "text-2xl font-semibold tabular-nums mt-1 text-[var(--accent)]"
            : "text-2xl font-mono tabular-nums mt-1 text-[var(--text-primary)] font-light"
        }
      >
        {value}
      </div>
      {hint && (
        <div className="text-[11px] text-[var(--text-tertiary)] mt-1">{hint}</div>
      )}
    </Card>
  );
}

function formatHoursFixed(hours: number): string {
  if (!Number.isFinite(hours) || hours <= 0) return "0";
  const rounded = Math.round(hours * 10) / 10;
  return Number.isInteger(rounded) ? `${rounded}` : rounded.toFixed(1);
}

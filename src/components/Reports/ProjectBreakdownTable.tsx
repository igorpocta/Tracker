/**
 * Per-project breakdown with hours, % of total, and earnings.
 *
 * "Project" is derived from the prefix of the issue key (`ACME-123` → `ACME`).
 * Sorted by earnings DESC when an hourly rate is configured, otherwise by
 * hours DESC. The Earnings column is hidden entirely when the rate is 0.
 */
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";
import {
  formatDurationShort,
  formatMoney,
} from "../../lib/format";

export interface ProjectBreakdownTableProps {
  rows: WorklogRow[];
  hourlyRate: number;
  currency: string;
}

interface ProjectRow {
  project: string;
  seconds: number;
  hours: number;
  earnings: number;
  percent: number;
}

export function ProjectBreakdownTable({
  rows,
  hourlyRate,
  currency,
}: ProjectBreakdownTableProps) {
  const showEarnings = hourlyRate > 0;

  const ranked = useMemo<ProjectRow[]>(() => {
    const totals = new Map<string, number>();
    for (const r of rows) {
      const key = (r.issue_key.split("-")[0] || "?").toUpperCase();
      totals.set(key, (totals.get(key) ?? 0) + r.duration_s);
    }
    const grand = Array.from(totals.values()).reduce((a, b) => a + b, 0) || 1;
    const arr: ProjectRow[] = Array.from(totals.entries()).map(
      ([project, seconds]) => {
        const hours = seconds / 3600;
        return {
          project,
          seconds,
          hours,
          earnings: hours * hourlyRate,
          percent: (seconds / grand) * 100,
        };
      },
    );
    arr.sort((a, b) =>
      showEarnings ? b.earnings - a.earnings : b.seconds - a.seconds,
    );
    return arr;
  }, [rows, hourlyRate, showEarnings]);

  if (ranked.length === 0) {
    return (
      <p className="text-xs text-[var(--text-tertiary)] py-6 text-center">
        No project data in range.
      </p>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="text-left text-[10px] uppercase tracking-[0.1em] text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
            <th className="px-2 py-2 font-medium">Project</th>
            <th className="px-2 py-2 font-medium text-right w-24">Hours</th>
            <th className="px-2 py-2 font-medium text-right w-16">%</th>
            {showEarnings && (
              <th className="px-2 py-2 font-medium text-right w-28">
                Earnings
              </th>
            )}
          </tr>
        </thead>
        <tbody>
          {ranked.map((row) => (
            <tr
              key={row.project}
              className="border-b border-[var(--border-subtle)] last:border-0 hover:bg-[var(--bg-hover)]"
            >
              <td className="px-2 py-2 font-mono text-[11px] uppercase text-[var(--text-secondary)]">
                {row.project}
              </td>
              <td className="px-2 py-2 text-right font-mono tabular-nums text-[var(--text-primary)]">
                {formatDurationShort(row.seconds)}
              </td>
              <td className="px-2 py-2 text-right font-mono tabular-nums text-[var(--text-tertiary)]">
                {row.percent.toFixed(0)}%
              </td>
              {showEarnings && (
                <td className="px-2 py-2 text-right font-mono tabular-nums text-[var(--text-primary)]">
                  {formatMoney(row.earnings, currency)}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

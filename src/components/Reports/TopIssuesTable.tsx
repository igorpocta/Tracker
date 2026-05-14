/**
 * Top-N issues table — issues ranked by total time logged in the range.
 */
import { useMemo } from "react";

import type { WorklogRow } from "../../api/types";
import { formatDurationShort } from "../../lib/format";

export interface TopIssuesTableProps {
  rows: WorklogRow[];
  /** Number of top issues to show. Defaults to 10. */
  limit?: number;
}

interface TopRow {
  issueKey: string;
  summary: string | null;
  totalSeconds: number;
  entries: number;
}

export function TopIssuesTable({ rows, limit = 10 }: TopIssuesTableProps) {
  const top = useMemo<TopRow[]>(() => {
    const map = new Map<string, TopRow>();
    for (const r of rows) {
      const existing = map.get(r.issue_key);
      if (existing) {
        existing.totalSeconds += r.duration_s;
        existing.entries += 1;
        if (!existing.summary && r.summary) existing.summary = r.summary;
      } else {
        map.set(r.issue_key, {
          issueKey: r.issue_key,
          summary: r.summary ?? null,
          totalSeconds: r.duration_s,
          entries: 1,
        });
      }
    }
    return Array.from(map.values())
      .sort((a, b) => b.totalSeconds - a.totalSeconds)
      .slice(0, limit);
  }, [rows, limit]);

  if (top.length === 0) {
    return (
      <p className="text-xs text-[var(--text-tertiary)] py-6 text-center">
        No issues logged in this range.
      </p>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="text-left text-[10px] uppercase tracking-[0.1em] text-[var(--text-tertiary)] border-b border-[var(--border-subtle)]">
            <th className="px-2 py-2 font-medium w-8">#</th>
            <th className="px-2 py-2 font-medium">Issue</th>
            <th className="px-2 py-2 font-medium text-right w-20">Entries</th>
            <th className="px-2 py-2 font-medium text-right w-24">Total</th>
          </tr>
        </thead>
        <tbody>
          {top.map((row, i) => (
            <tr
              key={row.issueKey}
              className="border-b border-[var(--border-subtle)] last:border-0 hover:bg-[var(--bg-hover)]"
            >
              <td className="px-2 py-2 text-[var(--text-tertiary)] font-mono tabular-nums">{i + 1}</td>
              <td className="px-2 py-2">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-[11px] uppercase text-[var(--text-secondary)] shrink-0">
                    {row.issueKey}
                  </span>
                  {row.summary && (
                    <span className="text-[var(--text-primary)] truncate">
                      {row.summary}
                    </span>
                  )}
                </div>
              </td>
              <td className="px-2 py-2 text-right text-[var(--text-tertiary)] font-mono tabular-nums">
                {row.entries}
              </td>
              <td className="px-2 py-2 text-right font-mono tabular-nums text-[var(--text-primary)]">
                {formatDurationShort(row.totalSeconds)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

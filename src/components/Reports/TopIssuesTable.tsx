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
      <p className="text-xs text-neutral-500 py-6 text-center">
        No issues logged in this range.
      </p>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="text-left text-[10px] uppercase tracking-wider text-neutral-500 border-b border-neutral-800">
            <th className="px-2 py-1.5 font-medium w-8">#</th>
            <th className="px-2 py-1.5 font-medium">Issue</th>
            <th className="px-2 py-1.5 font-medium text-right w-20">Entries</th>
            <th className="px-2 py-1.5 font-medium text-right w-24">Total</th>
          </tr>
        </thead>
        <tbody>
          {top.map((row, i) => (
            <tr
              key={row.issueKey}
              className="border-b border-neutral-800/60 last:border-0 hover:bg-neutral-800/30"
            >
              <td className="px-2 py-1.5 text-neutral-500 font-mono">{i + 1}</td>
              <td className="px-2 py-1.5">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-[11px] text-neutral-400 shrink-0">
                    {row.issueKey}
                  </span>
                  {row.summary && (
                    <span className="text-neutral-200 truncate">
                      {row.summary}
                    </span>
                  )}
                </div>
              </td>
              <td className="px-2 py-1.5 text-right text-neutral-400 font-mono tabular-nums">
                {row.entries}
              </td>
              <td className="px-2 py-1.5 text-right font-mono tabular-nums text-neutral-100">
                {formatDurationShort(row.totalSeconds)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

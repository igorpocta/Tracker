/**
 * Reports export button — XLSX přes SheetJS (`xlsx`).
 *
 * Sloupce:
 *   - Initiative — `{parent_key}: {parent_name}` z cache::issues_v2.
 *   - Issues     — `{issue_key}: {issue_summary}`.
 *   - Work start — JS Date (Excel ho zachytí jako native datetime cell).
 *   - Hours      — desetinné číslo (2 decimals).
 *
 * Soubor je `.xlsx` (native Excel binary), takže žádné kódování a oddělovače
 * neřešíme. Náhrada za starší TSV variantu, která Excel vyžadovala
 * importovat ručně.
 */
import { Download } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import * as XLSX from "xlsx";

import { searchIssuesCache } from "../../api/commands";
import type { IssueRow, WorklogRow } from "../../api/types";

export interface ExportButtonProps {
  rows: WorklogRow[];
  from: Date;
  to: Date;
}

export function ExportButton({ rows, from, to }: ExportButtonProps) {
  const issueKeys = Array.from(
    new Set(rows.map((r) => r.issue_key).filter((k): k is string => !!k)),
  );
  const issueQ = useQuery({
    queryKey: ["export-issues", issueKeys.sort().join(",")],
    enabled: issueKeys.length > 0,
    queryFn: async () => {
      const out = new Map<string, IssueRow>();
      for (const k of issueKeys) {
        try {
          const r = await searchIssuesCache(k, 5);
          const hit = r.find((row) => row.issue_key === k);
          if (hit) out.set(k, hit);
        } catch {
          /* ignore */
        }
      }
      return out;
    },
    staleTime: 60_000,
  });

  const handleExport = () => {
    const issueMap = issueQ.data ?? new Map<string, IssueRow>();
    const filename = `tracker-${formatIso(from)}-${formatIso(to)}.xlsx`;
    writeXlsx(rows, issueMap, filename);
  };

  return (
    <button
      type="button"
      onClick={handleExport}
      disabled={rows.length === 0}
      className="inline-flex items-center gap-1.5 px-3 h-8
                 rounded-[var(--radius-md)] text-xs text-[var(--accent)]
                 border border-[var(--accent-soft)]
                 bg-transparent hover:bg-[var(--accent-soft)]
                 transition-colors duration-150
                 disabled:opacity-60 disabled:cursor-not-allowed"
      title="Exportovat výkaz do Excelu (.xlsx)"
    >
      <Download className="w-3.5 h-3.5" aria-hidden />
      Exportovat do Excelu
    </button>
  );
}

/**
 * Build the XLSX file and trigger a browser download. Pure logic mimo
 * tohohle souboru je zachováno (`buildRowsForExport`) ať to lze
 * unit-testovat bez SheetJS závislosti.
 */
function writeXlsx(
  rows: WorklogRow[],
  issueMap: Map<string, IssueRow>,
  filename: string,
) {
  const headers = [
    "Initiative",
    "Issue",
    "Popis",
    "Začátek",
    "Hodiny",
  ];
  const dataRows = buildRowsForExport(rows, issueMap);

  const ws = XLSX.utils.aoa_to_sheet([
    headers,
    ...dataRows.map((r) => [
      r.initiative,
      r.issueLabel,
      r.description,
      r.start, // Date — Excel ho zachytí jako native datetime
      r.hours, // number → Excel "Number" cell, ne text
    ]),
  ]);

  // Šířky sloupců, ať se po otevření neprasknul layout.
  ws["!cols"] = [
    { wch: 26 },
    { wch: 32 },
    { wch: 40 },
    { wch: 20 },
    { wch: 10 },
  ];

  // Excel datetime formát pro sloupec "Začátek". Iterujeme buňky a pokud je
  // typ d (Date), nasadíme formát.
  const range = XLSX.utils.decode_range(ws["!ref"] ?? "A1");
  for (let R = range.s.r + 1; R <= range.e.r; R++) {
    const startCell = XLSX.utils.encode_cell({ r: R, c: 3 });
    const cell = ws[startCell];
    if (cell && cell.t === "d") {
      cell.z = "dd.mm.yyyy hh:mm";
    }
    const hoursCell = XLSX.utils.encode_cell({ r: R, c: 4 });
    const hc = ws[hoursCell];
    if (hc && hc.t === "n") {
      hc.z = "0.00";
    }
  }

  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, ws, "Worklogy");
  XLSX.writeFile(wb, filename, { compression: true });
}

interface ExportRow {
  initiative: string;
  issueLabel: string;
  description: string;
  start: Date;
  hours: number;
}

/** Pure helper — testovatelný bez DOM / SheetJS. */
export function buildRowsForExport(
  rows: WorklogRow[],
  issueMap: Map<string, IssueRow>,
): ExportRow[] {
  return rows
    .slice()
    .sort((a, b) => a.started_at - b.started_at)
    .map((r) => {
      const key = r.issue_key ?? "";
      const iss = key ? issueMap.get(key) : undefined;
      const initiative =
        iss?.parent_key && iss?.parent_summary
          ? `${iss.parent_key}: ${iss.parent_summary}`
          : "";
      const issueLabel = key
        ? `${key}: ${iss?.summary ?? r.summary ?? ""}`
        : "(bez úkolu)";
      const description = r.description ?? r.comment ?? "";
      const start = new Date(r.started_at * 1000);
      const hours = Math.round((r.duration_s / 3600) * 100) / 100;
      return { initiative, issueLabel, description, start, hours };
    });
}

function formatIso(d: Date): string {
  return `${d.getFullYear()}-${`${d.getMonth() + 1}`.padStart(2, "0")}-${`${d.getDate()}`.padStart(2, "0")}`;
}

/**
 * Pure-function tests pro Reports XLSX export.
 *
 * SheetJS samotný netestujeme; testujeme `buildRowsForExport`, který připravuje
 * data pro `XLSX.utils.aoa_to_sheet`.
 */
import { describe, expect, it } from "vitest";

import type { IssueRow, WorklogRow } from "../../api/types";
import { buildRowsForExport } from "./ExportButton";

function wl(
  issueKey: string | undefined,
  summary: string,
  startedAt: Date,
  durSec: number,
  description?: string,
): WorklogRow {
  return {
    issue_key: issueKey,
    duration_s: durSec,
    started_at: Math.floor(startedAt.getTime() / 1000),
    logged_at: 0,
    summary,
    description,
  };
}

function iss(
  key: string,
  summary: string,
  parentKey?: string,
  parentSummary?: string,
): IssueRow {
  return {
    issue_key: key,
    summary,
    updated_at: 0,
    parent_key: parentKey,
    parent_summary: parentSummary,
  };
}

describe("buildRowsForExport", () => {
  it("sortuje podle started_at vzestupně", () => {
    const rows = [
      wl("DEV-792", "Portál synchronizace", new Date(2026, 4, 14, 15, 46), 900),
      wl("DEV-304", "Úpravy", new Date(2026, 4, 14, 10, 0), 7200),
    ];
    const issueMap = new Map<string, IssueRow>([
      ["DEV-792", iss("DEV-792", "Portál synchronizace", "STREAM-4", "myDOCK")],
      ["DEV-304", iss("DEV-304", "Úpravy", "STREAM-4", "myDOCK")],
    ]);
    const out = buildRowsForExport(rows, issueMap);
    expect(out).toHaveLength(2);
    expect(out[0].issueLabel).toBe("DEV-304: Úpravy");
    expect(out[1].issueLabel).toBe("DEV-792: Portál synchronizace");
  });

  it("skládá Initiative z parent_key + parent_summary", () => {
    const rows = [wl("DEV-1", "x", new Date(2026, 4, 14, 9, 0), 1800)];
    const issueMap = new Map<string, IssueRow>([
      ["DEV-1", iss("DEV-1", "x", "EPIC-9", "Big epic")],
    ]);
    const out = buildRowsForExport(rows, issueMap);
    expect(out[0].initiative).toBe("EPIC-9: Big epic");
  });

  it("Initiative je prázdné, když parent chybí", () => {
    const rows = [wl("DEV-1", "x", new Date(2026, 4, 14, 9, 0), 1800)];
    const issueMap = new Map<string, IssueRow>([["DEV-1", iss("DEV-1", "x")]]);
    const out = buildRowsForExport(rows, issueMap);
    expect(out[0].initiative).toBe("");
  });

  it("fallback na row.summary, když issue není v cache", () => {
    const rows = [wl("DEV-1", "fallback summary", new Date(2026, 4, 14, 9, 0), 1800)];
    const out = buildRowsForExport(rows, new Map());
    expect(out[0].issueLabel).toBe("DEV-1: fallback summary");
  });

  it("(bez úkolu) když issue_key chybí", () => {
    const rows = [wl(undefined, "x", new Date(2026, 4, 14, 9, 0), 1800)];
    const out = buildRowsForExport(rows, new Map());
    expect(out[0].issueLabel).toBe("(bez úkolu)");
  });

  it("hours na 2 desetinná místa", () => {
    expect(buildRowsForExport([wl("X", "x", new Date(), 15 * 60)], new Map())[0].hours).toBe(0.25);
    expect(buildRowsForExport([wl("X", "x", new Date(), 2 * 3600 + 13 * 60)], new Map())[0].hours).toBe(2.22);
    expect(buildRowsForExport([wl("X", "x", new Date(), 9792)], new Map())[0].hours).toBe(2.72);
  });

  it("start je Date z started_at × 1000", () => {
    const d = new Date(2026, 4, 14, 15, 46, 0);
    const out = buildRowsForExport([wl("X", "x", d, 3600)], new Map());
    expect(out[0].start.getTime()).toBe(d.getTime());
  });

  it("description preferuje description před comment", () => {
    const r: WorklogRow = {
      issue_key: "X",
      duration_s: 3600,
      started_at: 0,
      logged_at: 0,
      summary: "x",
      description: "popis-d",
      comment: "popis-c",
    };
    expect(buildRowsForExport([r], new Map())[0].description).toBe("popis-d");
  });
});

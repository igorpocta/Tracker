/**
 * Pure-function tests for the Reports TSV export — Phase 18B Item 11.
 *
 * We don't render the React component here; instead we exercise the
 * helpers that the user's spec hinges on:
 *   - formatHoursCs    → Czech decimal-comma hours
 *   - formatExcelTime  → `DD.MM.YYYY HH:MM:SS`
 *   - buildTsv         → tab-separated rows with the right column order
 */
import { describe, expect, it } from "vitest";

import type { IssueRow, WorklogRow } from "../../api/types";
import { buildTsv, formatExcelTime, formatHoursCs } from "./ExportButton";

describe("formatHoursCs", () => {
  it("formats 15 minutes as 0,25", () => {
    expect(formatHoursCs(15 * 60)).toBe("0,25");
  });

  it("formats 2h 13m as 2,22", () => {
    expect(formatHoursCs(2 * 3600 + 13 * 60)).toBe("2,22");
  });

  it("rounds to 2 decimals", () => {
    // 9792s = 2.72h
    expect(formatHoursCs(9792)).toBe("2,72");
  });

  it("clamps negative to 0,00", () => {
    expect(formatHoursCs(-100)).toBe("0,00");
  });
});

describe("formatExcelTime", () => {
  it("emits DD.MM.YYYY HH:MM:SS", () => {
    const d = new Date(2026, 4, 14, 15, 46, 0);
    expect(formatExcelTime(d)).toBe("14.05.2026 15:46:00");
  });

  it("pads single-digit components", () => {
    const d = new Date(2026, 0, 3, 7, 5, 9);
    expect(formatExcelTime(d)).toBe("03.01.2026 07:05:09");
  });
});

describe("buildTsv", () => {
  function wl(
    issueKey: string,
    summary: string,
    startedAt: Date,
    durSec: number,
  ): WorklogRow {
    return {
      issue_key: issueKey,
      duration_s: durSec,
      started_at: Math.floor(startedAt.getTime() / 1000),
      logged_at: 0,
      summary,
    };
  }

  function iss(
    key: string,
    summary: string,
    epicKey?: string,
    epicSummary?: string,
  ): IssueRow {
    return {
      issue_key: key,
      summary,
      updated_at: 0,
      epic_key: epicKey,
      epic_summary: epicSummary,
    };
  }

  it("emits header + tab-separated rows, sorted by started_at", () => {
    const rows = [
      wl("DEV-792", "Portál synchronizace", new Date(2026, 4, 14, 15, 46), 900),
      wl("DEV-304", "Úpravy", new Date(2026, 4, 14, 10, 0), 7200),
    ];
    const issueMap = new Map<string, IssueRow>([
      ["DEV-792", iss("DEV-792", "Portál synchronizace", "STREAM-4", "myDOCK")],
      ["DEV-304", iss("DEV-304", "Úpravy", "STREAM-4", "myDOCK")],
    ]);
    const tsv = buildTsv(rows, issueMap);
    const lines = tsv.split("\r\n");
    expect(lines[0]).toBe("Initiative\tIssues\tWork start time\tTime spent (hours)");
    // Sorted by started_at — DEV-304 (10:00) first.
    expect(lines[1]).toBe(
      "STREAM-4: myDOCK\tDEV-304: Úpravy\t14.05.2026 10:00:00\t2,00",
    );
    expect(lines[2]).toBe(
      "STREAM-4: myDOCK\tDEV-792: Portál synchronizace\t14.05.2026 15:46:00\t0,25",
    );
  });

  it("falls back to row summary when issue is uncached", () => {
    const rows = [wl("DEV-1", "fallback", new Date(2026, 4, 14, 9, 0), 1800)];
    const tsv = buildTsv(rows, new Map());
    const lines = tsv.split("\r\n");
    expect(lines[1]).toBe("\tDEV-1: fallback\t14.05.2026 09:00:00\t0,50");
  });
});

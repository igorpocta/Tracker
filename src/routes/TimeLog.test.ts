/**
 * Tests for the duration parser used by the inline-edit affordances on each
 * worklog row.
 */
import { describe, expect, it } from "vitest";

import { parseDurationToSeconds } from "./TimeLog";

describe("parseDurationToSeconds", () => {
  it("parses 'Xh Ym' combo strings", () => {
    expect(parseDurationToSeconds("1h 30m")).toBe(5400);
    expect(parseDurationToSeconds("2h")).toBe(7200);
    expect(parseDurationToSeconds("45m")).toBe(2700);
  });

  it("treats a bare integer as minutes", () => {
    expect(parseDurationToSeconds("60")).toBe(3600);
    expect(parseDurationToSeconds("15")).toBe(900);
  });

  it("accepts decimal hour values", () => {
    expect(parseDurationToSeconds("1.5h")).toBe(5400);
    expect(parseDurationToSeconds("0.25h")).toBe(900);
  });

  it("treats comma as decimal separator", () => {
    expect(parseDurationToSeconds("1,5h")).toBe(5400);
  });

  it("returns null for malformed input", () => {
    expect(parseDurationToSeconds("")).toBeNull();
    expect(parseDurationToSeconds("nonsense")).toBeNull();
    expect(parseDurationToSeconds("hi")).toBeNull();
  });
});

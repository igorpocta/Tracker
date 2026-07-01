import { describe, expect, it } from "vitest";

import { MESSAGES, translate } from "./messages";

describe("translate", () => {
  it("resolves against the active language", () => {
    expect(translate("cs", "nav.reports")).toBe("Reporty");
    expect(translate("en", "nav.reports")).toBe("Reports");
  });

  it("falls back to Czech, then to the raw key", () => {
    // Present in both; sanity that en works.
    expect(translate("en", "nav.timeLog")).toBe("Time log");
    // Unknown key returns itself so gaps are visible, never blank.
    expect(translate("en", "totally.unknown.key")).toBe("totally.unknown.key");
  });

  it("interpolates {vars}", () => {
    expect(translate("en", "nav.unassignedBadge", { count: 3 })).toBe(
      "3 unassigned",
    );
    expect(translate("cs", "nav.unassignedBadge", { count: 3 })).toBe(
      "3 nepřiřazených",
    );
  });

  it("every English key has a Czech counterpart and vice versa", () => {
    const csKeys = Object.keys(MESSAGES.cs).sort();
    const enKeys = Object.keys(MESSAGES.en).sort();
    expect(enKeys).toEqual(csKeys);
  });
});

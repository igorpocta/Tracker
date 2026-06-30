/**
 * Unit coverage for the day-mode header label in the "Časový záznam" screen.
 *
 * The label gained the Czech weekday name so the user can tell which day a
 * page is. Plain days lead with the capitalised weekday; relative days keep
 * the "Dnes/Včera/Zítra" prefix and show the weekday lowercased after it.
 *
 * 14. 5. 2026 is a Thursday (čtvrtek).
 */
import { describe, expect, it } from "vitest";

import { dayHeaderLabel } from "./TimeLog";

const thursday = new Date(2026, 4, 14); // Thu 14. 5. 2026

describe("dayHeaderLabel", () => {
  it("leads with the capitalised weekday for a plain (non-relative) day", () => {
    const today = new Date(2026, 4, 20);
    expect(dayHeaderLabel(thursday, today)).toBe("Čtvrtek · 14. 5. 2026");
  });

  it("keeps the Dnes prefix and lowercases the weekday for today", () => {
    const today = new Date(2026, 4, 14);
    expect(dayHeaderLabel(thursday, today)).toBe("Dnes · čtvrtek · 14. 5. 2026");
  });

  it("uses the Včera prefix for yesterday", () => {
    const today = new Date(2026, 4, 15);
    expect(dayHeaderLabel(thursday, today)).toBe("Včera · čtvrtek · 14. 5. 2026");
  });

  it("uses the Zítra prefix for tomorrow", () => {
    const today = new Date(2026, 4, 13);
    expect(dayHeaderLabel(thursday, today)).toBe("Zítra · čtvrtek · 14. 5. 2026");
  });
});

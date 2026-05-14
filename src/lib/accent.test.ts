/**
 * Palette tests — make sure switching palettes writes the right CSS variables
 * to the document root.
 */
import { afterEach, describe, expect, it } from "vitest";

import {
  applyPalette,
  DEFAULT_PALETTE_ID,
  DUAL_PALETTES,
  getPaletteSpec,
  isDualPalette,
  MONO_PALETTES,
} from "./accent";

afterEach(() => {
  // Reset any custom properties we wrote.
  const root = document.documentElement.style;
  for (const prop of [
    "--accent",
    "--accent-hover",
    "--accent-soft",
    "--accent-strong",
    "--accent-ring",
    "--accent-2",
    "--accent-2-soft",
    "--accent-h",
    "--accent-s",
    "--accent-l",
  ]) {
    root.removeProperty(prop);
  }
});

describe("palette specs", () => {
  it("has 10 mono palettes and 5 dual palettes", () => {
    expect(MONO_PALETTES).toHaveLength(10);
    expect(DUAL_PALETTES).toHaveLength(5);
  });

  it("identifies the mode correctly", () => {
    expect(isDualPalette("aurora")).toBe(false);
    expect(isDualPalette("love")).toBe(false);
    expect(isDualPalette("czech")).toBe(true);
    expect(isDualPalette("nordic-fjord")).toBe(true);
  });

  it("falls back to the default for unknown ids", () => {
    const spec = getPaletteSpec("does-not-exist");
    expect(spec.id).toBe(DEFAULT_PALETTE_ID);
  });
});

describe("applyPalette", () => {
  it("writes the primary color to --accent", () => {
    applyPalette("love"); // #EC4899
    const v = document.documentElement.style.getPropertyValue("--accent").trim();
    expect(v.toLowerCase()).toBe("#ec4899");
  });

  it("uses the primary for --accent-2 in mono mode", () => {
    applyPalette("trcker"); // #EAB308
    const a = document.documentElement.style.getPropertyValue("--accent").trim();
    const a2 = document.documentElement.style.getPropertyValue("--accent-2").trim();
    expect(a.toLowerCase()).toBe("#eab308");
    expect(a2.toLowerCase()).toBe("#eab308");
  });

  it("uses different colors for primary/secondary in dual mode", () => {
    applyPalette("czech"); // #3B82F6 + #DC2626
    const a = document.documentElement.style.getPropertyValue("--accent").trim();
    const a2 = document.documentElement.style.getPropertyValue("--accent-2").trim();
    expect(a.toLowerCase()).toBe("#3b82f6");
    expect(a2.toLowerCase()).toBe("#dc2626");
  });

  it("writes accent-soft as a translucent rgba", () => {
    applyPalette("aurora");
    const soft = document.documentElement.style.getPropertyValue("--accent-soft");
    expect(soft).toMatch(/^rgba\(20,\s*184,\s*166,\s*0\.15\)$/);
  });

  it("re-applies cleanly when switching between palettes", () => {
    applyPalette("love");
    applyPalette("halloween");
    const v = document.documentElement.style.getPropertyValue("--accent").trim();
    expect(v.toLowerCase()).toBe("#f97316");
  });
});

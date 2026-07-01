/**
 * Guards the accent-id lists against drift.
 *
 * There are four places that must agree on the set of palette ids:
 *   - src/lib/accent.ts        (MONO_PALETTES / DUAL_PALETTES)  ← source of truth
 *   - src/stores/prefsStore.ts (KNOWN_ACCENTS, via isAccentId)  ← hydration filter
 *   - src/api/types.ts         (AccentColor union)              ← compile-time only
 *   - src-tauri .../prefs.rs   (ALLOWED_ACCENTS)                ← backend validation
 *
 * If a palette exists in accent.ts but not in KNOWN_ACCENTS, hydration silently
 * resets the user's choice to the default on the next launch. This test pins
 * that every palette id is a recognised accent.
 */
import { describe, expect, it, vi } from "vitest";

import { coreMock } from "../test/__mocks__/tauri";

import { ALL_PALETTES } from "../lib/accent";
import { isAccentId } from "./prefsStore";

vi.mock("@tauri-apps/api/core", () => coreMock);

describe("accent-id lists stay in sync", () => {
  it("recognises every palette id from accent.ts as a known accent", () => {
    for (const p of ALL_PALETTES) {
      expect(isAccentId(p.id)).toBe(true);
    }
  });

  it("still accepts the legacy hues and rejects nonsense", () => {
    expect(isAccentId("blue")).toBe(true);
    expect(isAccentId("graphite")).toBe(true);
    expect(isAccentId("puce")).toBe(false);
  });
});

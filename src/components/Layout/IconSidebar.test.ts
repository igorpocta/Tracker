/**
 * Tests for the `formatCacheCount` helper used by the sidebar ring.
 *
 * Phase 18B — Item 5: the chip now uses Nk+ for >= 1000.
 */
import { describe, expect, it } from "vitest";

import { formatCacheCount } from "./IconSidebar";

describe("formatCacheCount", () => {
  it("renders dash for empty / negative / zero", () => {
    expect(formatCacheCount(0)).toBe("–");
    expect(formatCacheCount(-1)).toBe("–");
    expect(formatCacheCount(Number.NaN)).toBe("–");
  });

  it("returns exact counts under 1000", () => {
    expect(formatCacheCount(1)).toBe("1");
    expect(formatCacheCount(42)).toBe("42");
    expect(formatCacheCount(999)).toBe("999");
  });

  it("formats thousands as Nk+", () => {
    expect(formatCacheCount(1000)).toBe("1k+");
    expect(formatCacheCount(1999)).toBe("1k+");
    expect(formatCacheCount(2000)).toBe("2k+");
    expect(formatCacheCount(2499)).toBe("2k+");
    expect(formatCacheCount(9999)).toBe("9k+");
  });

  it("caps at 10k+", () => {
    expect(formatCacheCount(10000)).toBe("10k+");
    expect(formatCacheCount(50000)).toBe("10k+");
  });
});

import { describe, expect, it } from "vitest";

import { formatRemaining } from "./focus";

describe("formatRemaining", () => {
  it("pads minutes and seconds under an hour", () => {
    expect(formatRemaining(90)).toBe("1:30");
    expect(formatRemaining(5)).toBe("0:05");
    expect(formatRemaining(0)).toBe("0:00");
  });

  it("adds an hours segment once past 3600s", () => {
    expect(formatRemaining(3600)).toBe("1:00:00");
    expect(formatRemaining(3661)).toBe("1:01:01");
  });

  it("clamps negatives to zero rather than rendering a minus sign", () => {
    // The backend stops the session on expiry, but a clock skew between the
    // stored end time and the browser could briefly produce a negative.
    expect(formatRemaining(-5)).toBe("0:00");
  });

  it("floors fractional seconds", () => {
    expect(formatRemaining(59.9)).toBe("0:59");
  });
});

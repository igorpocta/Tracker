import { describe, expect, it } from "vitest";

import { eventToAccelerator, prettifyAccelerator } from "./shortcut";

type KeyLike = Parameters<typeof eventToAccelerator>[0];

function ev(partial: Partial<KeyLike>): KeyLike {
  return {
    code: "",
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...partial,
  } as KeyLike;
}

describe("eventToAccelerator", () => {
  it("maps Ctrl+Shift+letter to a Tauri accelerator", () => {
    expect(
      eventToAccelerator(ev({ code: "KeyP", ctrlKey: true, shiftKey: true })),
    ).toBe("CommandOrControl+Shift+P");
  });

  it("maps Cmd(meta)+Shift+Period", () => {
    expect(
      eventToAccelerator(ev({ code: "Period", metaKey: true, shiftKey: true })),
    ).toBe("CommandOrControl+Shift+Period");
  });

  it("maps Alt+digit", () => {
    expect(eventToAccelerator(ev({ code: "Digit1", altKey: true }))).toBe(
      "Alt+1",
    );
  });

  it("returns null when only a modifier key is held (incomplete combo)", () => {
    expect(eventToAccelerator(ev({ code: "ShiftLeft", shiftKey: true }))).toBe(
      null,
    );
  });

  it("returns null without a Cmd/Ctrl/Alt modifier (Shift alone is too weak for a global bind)", () => {
    expect(eventToAccelerator(ev({ code: "KeyA", shiftKey: true }))).toBe(null);
  });
});

describe("prettifyAccelerator", () => {
  it("renders mac glyphs without separators", () => {
    expect(prettifyAccelerator("CommandOrControl+Shift+Period", true)).toBe(
      "⌘⇧.",
    );
  });

  it("renders windows/linux tokens joined with +", () => {
    expect(prettifyAccelerator("CommandOrControl+Shift+Period", false)).toBe(
      "Ctrl+Shift+.",
    );
  });

  it("shows a placeholder for the empty (disabled) accelerator", () => {
    expect(prettifyAccelerator("", true)).toBe("—");
  });
});

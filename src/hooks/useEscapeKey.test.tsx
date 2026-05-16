import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useEscapeKey } from "./useEscapeKey";

function Harness({
  onEscape,
  enabled = true,
}: {
  onEscape: () => void;
  enabled?: boolean;
}) {
  useEscapeKey(onEscape, enabled);
  return <div data-testid="harness" />;
}

describe("useEscapeKey", () => {
  it("fires handler on Escape", () => {
    const handler = vi.fn();
    render(<Harness onEscape={handler} />);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("ignores other keys", () => {
    const handler = vi.fn();
    render(<Harness onEscape={handler} />);

    fireEvent.keyDown(window, { key: "Enter" });
    fireEvent.keyDown(window, { key: "ArrowDown" });
    fireEvent.keyDown(window, { key: "a" });
    expect(handler).not.toHaveBeenCalled();
  });

  it("does not install when disabled", () => {
    const handler = vi.fn();
    render(<Harness onEscape={handler} enabled={false} />);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(handler).not.toHaveBeenCalled();
  });

  it("removes the listener on unmount", () => {
    const handler = vi.fn();
    const { unmount } = render(<Harness onEscape={handler} />);

    unmount();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(handler).not.toHaveBeenCalled();
  });
});

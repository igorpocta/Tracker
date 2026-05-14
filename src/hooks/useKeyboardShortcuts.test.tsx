import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useKeyboardShortcuts } from "./useKeyboardShortcuts";

function Harness({
  onRefresh,
  onOpenSettings,
}: {
  onRefresh?: () => void;
  onOpenSettings?: () => void;
}) {
  useKeyboardShortcuts({ onRefresh, onOpenSettings });
  return <div data-testid="harness" />;
}

describe("useKeyboardShortcuts", () => {
  it("calls onRefresh when Cmd/Ctrl+R is pressed", () => {
    const onRefresh = vi.fn();
    render(<Harness onRefresh={onRefresh} />);

    fireEvent.keyDown(window, { key: "r", metaKey: true });
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    expect(onRefresh).toHaveBeenCalled();
  });

  it("calls onOpenSettings when Cmd/Ctrl+, is pressed", () => {
    const onOpenSettings = vi.fn();
    render(<Harness onOpenSettings={onOpenSettings} />);

    fireEvent.keyDown(window, { key: ",", metaKey: true });
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    expect(onOpenSettings).toHaveBeenCalled();
  });

  it("ignores unmodified keys", () => {
    const onRefresh = vi.fn();
    const onOpenSettings = vi.fn();
    render(<Harness onRefresh={onRefresh} onOpenSettings={onOpenSettings} />);

    fireEvent.keyDown(window, { key: "r" });
    fireEvent.keyDown(window, { key: "," });
    expect(onRefresh).not.toHaveBeenCalled();
    expect(onOpenSettings).not.toHaveBeenCalled();
  });

  it("removes the listener on unmount", () => {
    const onRefresh = vi.fn();
    const { unmount } = render(<Harness onRefresh={onRefresh} />);

    unmount();
    fireEvent.keyDown(window, { key: "r", metaKey: true });
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    expect(onRefresh).not.toHaveBeenCalled();
  });
});

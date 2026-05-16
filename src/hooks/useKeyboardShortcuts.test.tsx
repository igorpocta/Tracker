import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useKeyboardShortcuts } from "./useKeyboardShortcuts";

interface HarnessProps {
  onRefresh?: () => void;
  onReindex?: () => void;
  onNewEntry?: () => void;
  onOpenSettings?: () => void;
}

function Harness(props: HarnessProps) {
  useKeyboardShortcuts(props);
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

  it("calls onReindex when Cmd/Ctrl+I is pressed", () => {
    const onReindex = vi.fn();
    render(<Harness onReindex={onReindex} />);

    fireEvent.keyDown(window, { key: "i", metaKey: true });
    fireEvent.keyDown(window, { key: "i", ctrlKey: true });
    // `hasPrimaryModifier` accepts only one modifier per platform
    // (Cmd on macOS, Ctrl elsewhere), so the matching modifier fires
    // once. We assert "called" rather than a specific count so the
    // test is platform-agnostic.
    expect(onReindex).toHaveBeenCalled();
  });

  it("calls onNewEntry when Cmd/Ctrl+N is pressed", () => {
    const onNewEntry = vi.fn();
    render(<Harness onNewEntry={onNewEntry} />);

    fireEvent.keyDown(window, { key: "n", metaKey: true });
    fireEvent.keyDown(window, { key: "N", ctrlKey: true });
    expect(onNewEntry).toHaveBeenCalled();
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
    const onNewEntry = vi.fn();
    render(<Harness onRefresh={onRefresh} onNewEntry={onNewEntry} />);

    fireEvent.keyDown(window, { key: "r" });
    fireEvent.keyDown(window, { key: "n" });
    expect(onRefresh).not.toHaveBeenCalled();
    expect(onNewEntry).not.toHaveBeenCalled();
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

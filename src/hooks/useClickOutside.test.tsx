import { fireEvent, render } from "@testing-library/react";
import { useRef } from "react";
import { describe, expect, it, vi } from "vitest";

import { useClickOutside } from "./useClickOutside";

function Harness({
  onOutside,
  enabled = true,
}: {
  onOutside: () => void;
  enabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useClickOutside(ref, onOutside, enabled);
  return (
    <>
      <div ref={ref} data-testid="inside">
        <span data-testid="inside-child" />
      </div>
      <div data-testid="outside" />
    </>
  );
}

describe("useClickOutside", () => {
  it("fires handler on mousedown outside the ref element", () => {
    const handler = vi.fn();
    const { getByTestId } = render(<Harness onOutside={handler} />);

    fireEvent.mouseDown(getByTestId("outside"));
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("does NOT fire on mousedown inside the ref element", () => {
    const handler = vi.fn();
    const { getByTestId } = render(<Harness onOutside={handler} />);

    fireEvent.mouseDown(getByTestId("inside"));
    fireEvent.mouseDown(getByTestId("inside-child"));
    expect(handler).not.toHaveBeenCalled();
  });

  it("does not install the listener when disabled", () => {
    const handler = vi.fn();
    const { getByTestId } = render(
      <Harness onOutside={handler} enabled={false} />,
    );

    fireEvent.mouseDown(getByTestId("outside"));
    expect(handler).not.toHaveBeenCalled();
  });

  it("removes the listener on unmount", () => {
    const handler = vi.fn();
    const { getByTestId, unmount } = render(<Harness onOutside={handler} />);

    const outside = getByTestId("outside");
    unmount();
    fireEvent.mouseDown(outside);
    expect(handler).not.toHaveBeenCalled();
  });
});

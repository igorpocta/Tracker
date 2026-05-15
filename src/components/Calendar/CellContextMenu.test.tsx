/**
 * Tests for the Calendar day-cell context menu.
 *
 * Covers:
 *   - Renders "Označit jako nepracovní den" when day is working & not marked,
 *     and the reason picker fires `onMarkNonWorking` with the right reason.
 *   - Renders "Označit jako pracovní den" when day is already marked, and
 *     clicking it fires `onUnmark`.
 *   - Picking "Detail dne" calls `onOpenDetail`.
 *   - Escape closes the menu.
 */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CellContextMenu } from "./CellContextMenu";

function renderMenu(overrides: Partial<Parameters<typeof CellContextMenu>[0]> = {}) {
  const props = {
    x: 100,
    y: 100,
    date: "2026-05-15",
    isWorkingDay: true,
    isExplicitlyMarked: false,
    onMarkNonWorking: vi.fn(),
    onUnmark: vi.fn(),
    onOpenDetail: vi.fn(),
    onClose: vi.fn(),
    ...overrides,
  };
  render(<CellContextMenu {...props} />);
  return props;
}

describe("CellContextMenu", () => {
  it("shows the 'mark non-working' action for a regular working day and surfaces the reason picker", async () => {
    const user = userEvent.setup();
    const props = renderMenu();

    // Root menu.
    const markBtn = screen.getByRole("menuitem", {
      name: /Označit jako nepracovní den/i,
    });
    await user.click(markBtn);

    // Reason picker now visible.
    expect(
      screen.getByRole("menuitem", { name: /Dovolená/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("menuitem", { name: /Svátek/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("menuitem", { name: /Osobní/ }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("menuitem", { name: /Svátek/ }));

    expect(props.onMarkNonWorking).toHaveBeenCalledExactlyOnceWith("holiday");
    expect(props.onClose).toHaveBeenCalledTimes(1);
    expect(props.onUnmark).not.toHaveBeenCalled();
  });

  it("shows the 'mark working' action when the day is already non-working", async () => {
    const user = userEvent.setup();
    const props = renderMenu({
      isWorkingDay: false,
      isExplicitlyMarked: true,
    });

    const unmarkBtn = screen.getByRole("menuitem", {
      name: /Označit jako pracovní den/i,
    });
    await user.click(unmarkBtn);

    expect(props.onUnmark).toHaveBeenCalledTimes(1);
    expect(props.onClose).toHaveBeenCalledTimes(1);
    expect(props.onMarkNonWorking).not.toHaveBeenCalled();
  });

  it("calls onOpenDetail when 'Detail dne' is clicked", async () => {
    const user = userEvent.setup();
    const props = renderMenu();

    await user.click(screen.getByRole("menuitem", { name: /Detail dne/ }));

    expect(props.onOpenDetail).toHaveBeenCalledTimes(1);
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const props = renderMenu();

    await user.keyboard("{Escape}");
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });
});

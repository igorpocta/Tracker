/**
 * Windows custom titlebar controls.
 *
 * On Windows the native title bar is disabled (see lib.rs) because it clashes
 * with the app's dark chrome, so we draw our own bar with minimize / maximize /
 * close buttons wired to the Tauri window API.
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const win = {
  minimize: vi.fn(() => Promise.resolve()),
  toggleMaximize: vi.fn(() => Promise.resolve()),
  close: vi.fn(() => Promise.resolve()),
  startDragging: vi.fn(() => Promise.resolve()),
  isMaximized: vi.fn(() => Promise.resolve(false)),
  onResized: vi.fn(() => Promise.resolve(() => {})),
};

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => win,
}));

import { WindowTitlebar } from "./WindowTitlebar";

beforeEach(() => {
  Object.values(win).forEach((f) => f.mockClear?.());
});

describe("WindowTitlebar", () => {
  it("minimizes the window", async () => {
    const user = userEvent.setup();
    render(<WindowTitlebar />);
    await user.click(screen.getByRole("button", { name: /minimalizovat/i }));
    await waitFor(() => expect(win.minimize).toHaveBeenCalledTimes(1));
  });

  it("toggles maximize", async () => {
    const user = userEvent.setup();
    render(<WindowTitlebar />);
    await user.click(screen.getByRole("button", { name: /maximalizovat|obnovit/i }));
    await waitFor(() => expect(win.toggleMaximize).toHaveBeenCalledTimes(1));
  });

  it("closes the window", async () => {
    const user = userEvent.setup();
    render(<WindowTitlebar />);
    await user.click(screen.getByRole("button", { name: /zavřít/i }));
    await waitFor(() => expect(win.close).toHaveBeenCalledTimes(1));
  });
});

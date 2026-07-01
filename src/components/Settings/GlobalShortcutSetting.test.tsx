/**
 * Tests for the Settings control that views / rebinds / disables the global
 * timer-toggle shortcut.
 */
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import { GlobalShortcutSetting } from "./GlobalShortcutSetting";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

function mockStatus(accelerator: string, registered: boolean) {
  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === "get_global_shortcut") return { accelerator, registered };
    if (cmd === "set_global_shortcut") {
      const a = (args as { accelerator: string }).accelerator;
      return { accelerator: a, registered: a.trim().length > 0 };
    }
    return null;
  });
}

describe("GlobalShortcutSetting", () => {
  it("loads and offers to change the current shortcut", async () => {
    mockStatus("CommandOrControl+Shift+Period", true);
    render(<GlobalShortcutSetting />);
    await screen.findByRole("button", { name: /změnit/i });
    // A registered shortcut shows no warning.
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("warns when the stored shortcut is not registered (already taken)", async () => {
    mockStatus("CommandOrControl+Shift+Period", false);
    render(<GlobalShortcutSetting />);
    await screen.findByRole("alert");
  });

  it("records a new combo and persists it via set_global_shortcut", async () => {
    mockStatus("CommandOrControl+Shift+Period", true);
    const user = userEvent.setup();
    render(<GlobalShortcutSetting />);

    await user.click(await screen.findByRole("button", { name: /změnit/i }));

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          code: "KeyP",
          ctrlKey: true,
          shiftKey: true,
          bubbles: true,
        }),
      );
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_global_shortcut", {
        accelerator: "CommandOrControl+Shift+P",
      });
    });
  });

  it("disables the shortcut via the Vypnout button", async () => {
    mockStatus("CommandOrControl+Shift+Period", true);
    const user = userEvent.setup();
    render(<GlobalShortcutSetting />);

    await user.click(await screen.findByRole("button", { name: /vypnout/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_global_shortcut", {
        accelerator: "",
      });
    });
  });

  it("restores the default via the Obnovit výchozí button", async () => {
    mockStatus("", false); // currently disabled
    const user = userEvent.setup();
    render(<GlobalShortcutSetting />);

    await user.click(
      await screen.findByRole("button", { name: /obnovit výchozí/i }),
    );

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_global_shortcut", {
        accelerator: "CommandOrControl+Shift+Period",
      });
    });
  });
});

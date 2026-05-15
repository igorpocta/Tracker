/**
 * Tests for `Settings → Cíle` — working-week mask bitmask logic.
 *
 * The full SettingsGoals page involves the prefs store + a few queries we
 * don't want to spin up here, so the heart of the work-week mask is
 * extracted into the pure `toggleBit` helper which is what we test below.
 *
 * The component-level smoke (render mask checkboxes for Po..Ne) is covered
 * by rendering `WorkingWeekMask` directly with the Tauri invoke mocked.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import { toggleBit, WorkingWeekMask } from "./WorkingWeekMask";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe("toggleBit (work-week mask)", () => {
  it("unchecking Friday changes 31 → 15", () => {
    // 31 == 0b0011111 (Mon..Fri). Friday is bit 16.
    expect(toggleBit(31, 16)).toBe(15);
  });

  it("checking Saturday changes 31 → 63", () => {
    // Saturday is bit 32.
    expect(toggleBit(31, 32)).toBe(63);
  });

  it("toggling the same bit twice returns the original mask", () => {
    const start = 31;
    const once = toggleBit(start, 4); // toggle St
    const twice = toggleBit(once, 4);
    expect(twice).toBe(start);
  });
});

function renderMask() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <WorkingWeekMask />
    </QueryClientProvider>,
  );
}

describe("WorkingWeekMask component", () => {
  it("renders seven day checkboxes and writes the new bitmask on toggle", async () => {
    // First call: get_working_week_mask returns the default Mon..Fri.
    mockInvoke.mockResolvedValueOnce(31);
    // Second call (the toggle write) — resolves with no value.
    mockInvoke.mockResolvedValueOnce(undefined);

    const user = userEvent.setup();
    renderMask();

    // Wait for the initial fetch to land.
    const friday = await screen.findByLabelText("Pátek");
    expect(friday).toBeChecked();

    // Sanity: there are exactly seven checkboxes (Po..Ne).
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(7);

    await user.click(friday);

    await waitFor(() => {
      // The second invoke is the write. We don't care about the first arg
      // beyond noting that 15 (= 31 with the Friday bit cleared) is what
      // got sent.
      expect(mockInvoke).toHaveBeenCalledWith("set_working_week_mask", {
        mask: 15,
      });
    });
  });
});

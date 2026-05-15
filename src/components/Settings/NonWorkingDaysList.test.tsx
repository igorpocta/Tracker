/**
 * Tests for the non-working-days list under Settings → Cíle.
 *
 * - Renders one row per backend entry.
 * - Clicking the × button invokes `remove_non_working_day` with the row's date.
 * - When the backend returns an empty list, the empty-state copy is shown.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import { NonWorkingDaysList } from "./NonWorkingDaysList";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

function renderList() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NonWorkingDaysList />
    </QueryClientProvider>,
  );
}

describe("NonWorkingDaysList", () => {
  it("renders one row per non-working day and removes via the × button", async () => {
    // Initial list_non_working_days call.
    mockInvoke.mockImplementationOnce(async () => [
      {
        date: "2026-05-15",
        reason: "vacation",
        label: "Bali",
        created_at: 0,
      },
      {
        date: "2026-05-23",
        reason: "holiday",
        label: null,
        created_at: 0,
      },
    ]);
    // remove_non_working_day write.
    mockInvoke.mockResolvedValueOnce(undefined);
    // Refetch after invalidation.
    mockInvoke.mockResolvedValueOnce([
      {
        date: "2026-05-23",
        reason: "holiday",
        label: null,
        created_at: 0,
      },
    ]);

    const user = userEvent.setup();
    renderList();

    // Both rows render.
    await screen.findByText(/Bali/);
    expect(screen.getByText("15.05.2026")).toBeInTheDocument();
    expect(screen.getByText("23.05.2026")).toBeInTheDocument();

    const removeBtn = screen.getByRole("button", {
      name: /Odebrat 15.05.2026/,
    });
    await user.click(removeBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("remove_non_working_day", {
        date: "2026-05-15",
      });
    });
  });

  it("shows the empty-state copy when the list is empty", async () => {
    mockInvoke.mockResolvedValueOnce([]);

    renderList();

    await screen.findByText(
      /Žádné nepracovní dny v rozsahu posledních 30 a příštích 90 dnů/,
    );
  });
});

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

  it("does not render pagination when the list fits on one page", async () => {
    mockInvoke.mockResolvedValueOnce([
      {
        date: "2026-05-15",
        reason: "vacation",
        label: "Bali",
        created_at: 0,
      },
    ]);

    renderList();

    await screen.findByText(/Bali/);
    expect(
      screen.queryByRole("navigation", { name: /Stránkování/ }),
    ).not.toBeInTheDocument();
  });

  it("sorts by date descending and paginates at 20 rows per page", async () => {
    // 25 entries, dates 2026-02-01 … 2026-02-25 (ascending with index i);
    // labels zero-padded so substring matchers can't collide (Day-05 ≠ Day-15).
    const days = Array.from({ length: 25 }, (_, i) => ({
      date: `2026-02-${String(i + 1).padStart(2, "0")}`,
      reason: "personal",
      label: `Day-${String(i).padStart(2, "0")}`,
      created_at: i,
    }));
    mockInvoke.mockResolvedValueOnce(days);

    const user = userEvent.setup();
    renderList();

    // Descending: the newest date (25.02) is the FIRST row.
    await screen.findByText("25.02.2026");
    const dates = screen.getAllByText(/^\d{2}\.\d{2}\.2026$/);
    expect(dates[0].textContent).toBe("25.02.2026");

    // Page 1 = the 20 newest (Day-24 … Day-05); Day-04 is on page 2.
    expect(screen.getByText(/Day-24/)).toBeInTheDocument();
    expect(screen.getByText(/Day-05/)).toBeInTheDocument();
    expect(screen.queryByText(/Day-04/)).not.toBeInTheDocument();

    expect(
      screen.getByRole("navigation", { name: /Stránkování/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/1 \/ 2/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Další/ }));

    // Page 2 = the 5 oldest (Day-04 … Day-00); 01.02 is the last date.
    await screen.findByText(/Day-00/);
    expect(screen.getByText("01.02.2026")).toBeInTheDocument();
    expect(screen.queryByText(/Day-24/)).not.toBeInTheDocument();
    expect(screen.getByText(/2 \/ 2/)).toBeInTheDocument();
  });

  it("hides pagination at exactly 20 rows (≤ 20 shows no pager)", async () => {
    const days = Array.from({ length: 20 }, (_, i) => ({
      date: `2026-02-${String(i + 1).padStart(2, "0")}`,
      reason: "personal",
      label: `Day-${String(i).padStart(2, "0")}`,
      created_at: i,
    }));
    mockInvoke.mockResolvedValueOnce(days);

    renderList();

    await screen.findByText(/Day-00/);
    expect(
      screen.queryByRole("navigation", { name: /Stránkování/ }),
    ).not.toBeInTheDocument();
  });
});

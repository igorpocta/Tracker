/**
 * Tests for SuggestionBanner accept flow.
 *
 * Regression: "Spustit" called the raw startTimer command and swallowed
 * errors, so a failure gave no feedback and a missed `timer-started` event
 * left the store (and the whole UI) showing no running timer while the backend
 * had one. Accept now goes through the timer store (updates `active` directly)
 * and toasts on error.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";
import { useTimerStore } from "../../stores/timerStore";

vi.mock("@tauri-apps/api/core", () => coreMock);

import { SuggestionBanner } from "./SuggestionBanner";

const PUSH_TOAST = vi.fn();
const SHELL_CTX = {
  pushToast: PUSH_TOAST,
  openStopDialog: vi.fn(),
  openAddEntry: vi.fn(),
};

const SUGGESTION = {
  issue_key: "DEV-1",
  summary: "Fix login",
  bucket_hour: 9,
  occurrences: 3,
};

function arrange(startTimerImpl: () => Promise<unknown>) {
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_smart_suggestions_enabled":
        return Promise.resolve(true);
      case "get_suggestions":
        return Promise.resolve([SUGGESTION]);
      case "start_timer":
        return startTimerImpl();
      default:
        return Promise.resolve(null);
    }
  });
}

function renderBanner() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/"]}>
        <Routes>
          <Route element={<Outlet context={SHELL_CTX} />}>
            <Route path="/" element={<SuggestionBanner />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("<SuggestionBanner /> accept", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    PUSH_TOAST.mockReset();
    useTimerStore.setState({ active: null, busy: false, error: null });
  });

  it("starts via the timer store so `active` is set on success", async () => {
    arrange(() =>
      Promise.resolve({ issue_key: "DEV-1", started_at: 123, elapsed_seconds: 0 }),
    );
    const user = userEvent.setup();
    renderBanner();

    await user.click(await screen.findByRole("button", { name: /Spustit/ }));

    await waitFor(() =>
      expect(useTimerStore.getState().active?.issue_key).toBe("DEV-1"),
    );
  });

  it("toasts an error when starting fails", async () => {
    arrange(() => Promise.reject("boom"));
    const user = userEvent.setup();
    renderBanner();

    await user.click(await screen.findByRole("button", { name: /Spustit/ }));

    await waitFor(() =>
      expect(PUSH_TOAST).toHaveBeenCalledWith("error", expect.any(String)),
    );
  });
});

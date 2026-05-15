/**
 * Tests for the controlled/uncontrolled split on <FavoriteStar />.
 *
 * Regression target: previously the component ran an `isFavorite(key)`
 * IPC query unconditionally — even when the parent (StartTrackingBar
 * dropdown) had already derived the answer from its `favorites` cache
 * and passed it in via `initial`. With 20 dropdown rows that meant 20
 * redundant Tauri round-trips per open.
 *
 * Fix: `initial` provided → controlled mode, no query. `initial`
 * omitted → uncontrolled, query runs.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { FavoriteStar } from "./FavoriteStar";

vi.mock("../../api/commands", async () => {
  const real = await vi.importActual<typeof commands>(
    "../../api/commands",
  );
  return {
    ...real,
    isFavorite: vi.fn(),
    addFavorite: vi.fn(),
    removeFavorite: vi.fn(),
  };
});

const isFavoriteMock = vi.mocked(commands.isFavorite);

afterEach(() => {
  isFavoriteMock.mockReset();
});

function withProviders(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={client}>{node}</QueryClientProvider>);
}

describe("<FavoriteStar /> controlled vs uncontrolled", () => {
  it("does NOT issue an IPC when the parent passes `initial`", async () => {
    isFavoriteMock.mockResolvedValue(true);

    withProviders(<FavoriteStar issueKey="ACME-1" initial={true} />);

    // Render shows the controlled state immediately.
    const button = await screen.findByRole("button", {
      name: "Odebrat z oblíbených",
    });
    expect(button).toBeInTheDocument();

    // Give react-query a tick to fire any pending fetches (it
    // shouldn't, but the assertion is meaningful precisely because
    // we waited).
    await new Promise((r) => setTimeout(r, 20));

    expect(isFavoriteMock).not.toHaveBeenCalled();
  });

  it("renders the un-favorited state from `initial={false}` without an IPC", async () => {
    isFavoriteMock.mockResolvedValue(true);

    withProviders(<FavoriteStar issueKey="ACME-2" initial={false} />);

    const button = await screen.findByRole("button", {
      name: "Přidat do oblíbených",
    });
    expect(button).toBeInTheDocument();

    await new Promise((r) => setTimeout(r, 20));

    expect(isFavoriteMock).not.toHaveBeenCalled();
  });

  it("DOES issue an IPC when no `initial` is provided", async () => {
    isFavoriteMock.mockResolvedValue(true);

    withProviders(<FavoriteStar issueKey="ACME-3" />);

    await waitFor(() => {
      expect(isFavoriteMock).toHaveBeenCalledWith("ACME-3");
    });
  });
});

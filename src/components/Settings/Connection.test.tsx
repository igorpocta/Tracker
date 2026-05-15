/**
 * Tests for Settings → Připojení (Phase 18F).
 *
 * Covered:
 *   - Single card per connection: even when both a legacy
 *     `get_current_config` and a matching `list_connections` row exist for
 *     the same account, the UI must NOT render two cards (the legacy path
 *     has been removed entirely).
 *   - Clicking "Přidat nové připojení" opens the inline dialog and the
 *     provider picker is visible.
 *   - The dialog routes the user through the Jira credentials form when
 *     "Jira" is picked.
 *   - The inline rename flow: pencil icon → input → Enter → emits
 *     `update_connection` with the new name.
 *   - The remove flow confirms then dispatches `remove_connection`.
 */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import Connection from "./Connection";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0, staleTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <Connection />
    </QueryClientProvider>,
  );
}

/**
 * Helper that handles every command Connection.tsx fires on mount and
 * returns a controllable `list_connections` resolver so individual tests can
 * arrange the seeded data.
 */
function mockListConnections(connections: unknown[]) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "list_connections") return connections;
    return null;
  });
}

describe("Settings → Připojení", () => {
  it("renders a single card per backend connection (no legacy duplicate)", async () => {
    // The defect: pre-Phase-18F we used to render BOTH the legacy
    // `get_current_config` Jira row AND the matching `list_connections`
    // row, producing two cards. The new component never calls
    // `get_current_config` and renders only one card here.
    mockListConnections([
      {
        id: 1,
        provider: "jira",
        name: "Pocta, Jira - atlassian",
        enabled: true,
        created_at: 0,
        updated_at: 0,
        config: {
          base_url: "https://sabservis.atlassian.net",
          email: "pocta@sabservis.cz",
        },
        has_token: true,
      },
    ]);

    renderPanel();

    await screen.findByText(/Pocta, Jira - atlassian/);
    // Exactly one card.
    expect(screen.getAllByTestId(/connection-card-/)).toHaveLength(1);
    // We must NOT have fired `get_current_config`. (Belt and braces: the
    // component never imports it, but enforce it here so regressions surface.)
    const cmds = mockInvoke.mock.calls.map((c) => c[0]);
    expect(cmds).not.toContain("get_current_config");
  });

  it("opens the inline Add dialog when 'Přidat nové připojení' is clicked", async () => {
    mockListConnections([]);

    const user = userEvent.setup();
    renderPanel();

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    const addButton = screen.getByTestId("add-connection-button");
    await user.click(addButton);

    // The provider picker appears INSIDE a dialog (not a navigation away).
    expect(screen.getByTestId("add-connection-dialog")).toBeInTheDocument();
    // The "Vyberte poskytovatele" copy lives both in the dialog header and
    // the StepProvider form label, so just count instances rather than
    // grab one.
    expect(screen.getAllByText(/vyberte poskytovatele/i).length).toBeGreaterThan(0);
    expect(screen.getByTestId("provider-card-jira")).toBeInTheDocument();
    expect(screen.getByTestId("provider-card-freelo")).toBeInTheDocument();
  });

  it("after picking Jira shows the credentials form with a name field", async () => {
    mockListConnections([]);

    const user = userEvent.setup();
    renderPanel();

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    await user.click(screen.getByTestId("add-connection-button"));
    await user.click(screen.getByTestId("provider-card-jira"));
    await user.click(screen.getByRole("button", { name: /^další$/i }));

    // Name field is pre-filled with "Jira" and editable.
    const nameInput = screen.getByLabelText(/název připojení/i) as HTMLInputElement;
    expect(nameInput.value).toBe("Jira");

    // URL + email + token inputs are present.
    expect(screen.getByLabelText(/základní url jiry/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/e-mail atlassian účtu/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/jira api token/i)).toBeInTheDocument();
  });

  it("rename flow: edit name → Enter → invokes update_connection with new name", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_connections") {
        return [
          {
            id: 7,
            provider: "jira",
            name: "Jira",
            enabled: true,
            created_at: 0,
            updated_at: 0,
            config: {
              base_url: "https://acme.atlassian.net",
              email: "user@acme.test",
            },
            has_token: true,
          },
        ];
      }
      if (cmd === "update_connection") return undefined;
      return null;
    });

    const user = userEvent.setup();
    renderPanel();

    await screen.findByText("Jira");

    // Click the name to switch into rename mode.
    await user.click(screen.getByTestId("conn-name-7"));
    const input = screen.getByTestId("conn-rename-input") as HTMLInputElement;
    expect(input).toHaveFocus();

    await user.clear(input);
    await user.type(input, "SAB");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_connection", {
        args: expect.objectContaining({ id: 7, name: "SAB" }),
      });
    });
  });

  it("remove flow: confirms then dispatches remove_connection", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_connections") {
        return [
          {
            id: 3,
            provider: "freelo",
            name: "Můj Freelo",
            enabled: true,
            created_at: 0,
            updated_at: 0,
            config: { email: "u@x.test" },
            has_token: true,
          },
        ];
      }
      return null;
    });

    const confirmSpy = vi
      .spyOn(window, "confirm")
      .mockReturnValue(true);

    const user = userEvent.setup();
    renderPanel();

    await screen.findByText("Můj Freelo");

    await user.click(screen.getByTestId("conn-remove-3"));

    expect(confirmSpy).toHaveBeenCalled();
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("remove_connection", { id: 3 });
    });

    confirmSpy.mockRestore();
  });
});

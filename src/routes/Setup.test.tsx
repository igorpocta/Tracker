/**
 * Vitest coverage for the 3-step setup wizard.
 *
 * We mock `@tauri-apps/api/core` at the module level so the typed wrappers in
 * `src/api/commands.ts` end up calling our spy. That way the tests exercise
 * the real wrapper code path (param shape, command name) instead of stubbing
 * one layer up.
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { coreMock, mockInvoke } from "../test/__mocks__/tauri";
import Setup from "./Setup";

vi.mock("@tauri-apps/api/core", () => coreMock);

function renderSetup() {
  return render(
    <MemoryRouter initialEntries={["/setup"]}>
      <Routes>
        <Route path="/setup" element={<Setup />} />
        <Route
          path="/"
          element={<div data-testid="home-marker">home</div>}
        />
      </Routes>
    </MemoryRouter>,
  );
}

/**
 * Helper: drive past the new provider-picker step (step 0) into the Jira
 * flow so existing tests can keep their step numbering ("URL is step 1").
 */
async function selectJiraProvider(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByTestId("provider-card-jira"));
  await user.click(screen.getByRole("button", { name: /^další$/i }));
}

describe("Setup wizard", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("starts on the provider picker (step 1 of 4)", () => {
    renderSetup();
    expect(screen.getByText(/vyberte poskytovatele/i)).toBeInTheDocument();
    // Now showing 4 steps for Jira (picker + url + email + token).
    expect(screen.getByText(/krok 1 z 4/i)).toBeInTheDocument();
  });

  it("shows the URL step after picking Jira", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);
    expect(screen.getByLabelText(/základní url jiry/i)).toBeInTheDocument();
    expect(screen.getByText(/krok 2 z 4/i)).toBeInTheDocument();
  });

  it("keeps Další disabled until URL is valid https://", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);

    const input = screen.getByLabelText(/základní url jiry/i);
    const next = screen.getByRole("button", { name: /^další$/i });
    expect(next).toBeDisabled();

    await user.type(input, "http://nope");
    expect(next).toBeDisabled();
    expect(screen.getByText(/musí začínat https/i)).toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "https://acme.atlassian.net");
    expect(next).toBeEnabled();
  });

  it("advances through the steps with valid input", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);

    // Step 2 → 3.
    await user.type(
      screen.getByLabelText(/základní url jiry/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));

    // Step 3.
    expect(screen.getByText(/krok 3 z 4/i)).toBeInTheDocument();
    const emailInput = screen.getByLabelText(/e-mail atlassian/i);
    const next2 = screen.getByRole("button", { name: /^další$/i });
    expect(next2).toBeDisabled();
    await user.type(emailInput, "not-an-email");
    expect(next2).toBeDisabled();
    expect(screen.getByText(/musí být platný e-mail/i)).toBeInTheDocument();
    await user.clear(emailInput);
    await user.type(emailInput, "alice@example.com");
    expect(next2).toBeEnabled();
    await user.click(next2);

    // Step 4.
    expect(screen.getByText(/krok 4 z 4/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /otestovat připojení/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^dokončit$/i })).toBeDisabled();
  });

  it("enables Dokončit only after a successful connection test", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);

    // Drive through to step 4.
    await user.type(
      screen.getByLabelText(/základní url jiry/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    await user.type(
      screen.getByLabelText(/e-mail atlassian/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));

    // Test connection should be disabled until token meets min length.
    const testBtn = screen.getByRole("button", { name: /otestovat připojení/i });
    expect(testBtn).toBeDisabled();
    await user.type(screen.getByLabelText(/jira api token/i), "abcd1234567890");
    expect(testBtn).toBeEnabled();
    expect(screen.getByRole("button", { name: /^dokončit$/i })).toBeDisabled();

    // Mock the test_jira_connection call to succeed.
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_jira_connection") {
        return {
          accountId: "abc",
          displayName: "Alice Example",
          emailAddress: "alice@example.com",
        };
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    await user.click(testBtn);

    await waitFor(() =>
      expect(screen.getByText(/připojeno jako alice example/i)).toBeInTheDocument(),
    );

    // Verify the wrapper translated the JS args into the snake_case shape.
    expect(mockInvoke).toHaveBeenCalledWith("test_jira_connection", {
      baseUrl: "https://acme.atlassian.net",
      email: "alice@example.com",
      token: "abcd1234567890",
    });
    expect(screen.getByRole("button", { name: /^dokončit$/i })).toBeEnabled();
  });

  it("calls save_config and enter_main_app on Dokončit, then navigates home", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);

    await user.type(
      screen.getByLabelText(/základní url jiry/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    await user.type(
      screen.getByLabelText(/e-mail atlassian/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    await user.type(screen.getByLabelText(/jira api token/i), "abcd1234567890");

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "test_jira_connection":
          return {
            accountId: "abc",
            displayName: "Alice Example",
            emailAddress: "alice@example.com",
          };
        case "save_config":
        case "enter_main_app":
          return undefined;
        default:
          throw new Error(`unexpected command: ${cmd}`);
      }
    });

    await user.click(screen.getByRole("button", { name: /otestovat připojení/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^dokončit$/i })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: /^dokončit$/i }));

    await waitFor(() =>
      expect(screen.getByTestId("home-marker")).toBeInTheDocument(),
    );

    expect(mockInvoke).toHaveBeenCalledWith("save_config", {
      args: {
        config: {
          base_url: "https://acme.atlassian.net",
          email: "alice@example.com",
        },
        token: "abcd1234567890",
      },
    });
    expect(mockInvoke).toHaveBeenCalledWith("enter_main_app");
  });

  it("shows an error if the connection test fails", async () => {
    const user = userEvent.setup();
    renderSetup();
    await selectJiraProvider(user);

    await user.type(
      screen.getByLabelText(/základní url jiry/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    await user.type(
      screen.getByLabelText(/e-mail atlassian/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    await user.type(screen.getByLabelText(/jira api token/i), "abcd1234567890");

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_jira_connection") {
        throw "unauthorized";
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    await user.click(screen.getByRole("button", { name: /otestovat připojení/i }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/unauthorized/i),
    );
    expect(screen.getByRole("button", { name: /^dokončit$/i })).toBeDisabled();
  });

  // ----- Phase 18E: Freelo flow -----

  it("renders Freelo credentials when Freelo is picked", async () => {
    const user = userEvent.setup();
    renderSetup();
    await user.click(screen.getByTestId("provider-card-freelo"));
    await user.click(screen.getByRole("button", { name: /^další$/i }));
    expect(screen.getByLabelText(/freelo e-mail/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/freelo api klíč/i)).toBeInTheDocument();
    expect(screen.getByText(/krok 2 z 3/i)).toBeInTheDocument();
  });

  it("shows the project picker after a successful Freelo test+save", async () => {
    const user = userEvent.setup();
    renderSetup();
    await user.click(screen.getByTestId("provider-card-freelo"));
    await user.click(screen.getByRole("button", { name: /^další$/i }));

    await user.type(
      screen.getByLabelText(/freelo e-mail/i),
      "alice@example.com",
    );
    await user.type(
      screen.getByLabelText(/freelo api klíč/i),
      "abcdefghij1234567890",
    );

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "test_connection_for_provider":
          return {
            accountId: "7",
            displayName: "Alice Example",
            emailAddress: "alice@example.com",
            provider: "freelo",
          };
        case "add_connection":
          return {
            id: 1,
            provider: "freelo",
            name: "Freelo · Alice Example",
            enabled: true,
            created_at: 0,
            updated_at: 0,
            config: {},
            has_token: true,
          };
        case "list_freelo_projects":
          return [
            { id: 1, name: "Web", state: "active", selected: false },
            { id: 2, name: "Mobile", state: "active", selected: false },
          ];
        default:
          throw new Error(`unexpected command: ${cmd}`);
      }
    });

    await user.click(
      screen.getByRole("button", { name: /otestovat připojení/i }),
    );

    // Should advance to the project picker step.
    await waitFor(() =>
      expect(screen.getByTestId("freelo-projects-list")).toBeInTheDocument(),
    );
    expect(screen.getByText(/krok 3 z 3/i)).toBeInTheDocument();
    expect(screen.getByTestId("freelo-project-1")).toBeInTheDocument();
    expect(screen.getByTestId("freelo-project-2")).toBeInTheDocument();

    // Initially nothing selected → Finish disabled.
    expect(
      screen.getByRole("button", { name: /^dokončit$/i }),
    ).toBeDisabled();

    await user.click(screen.getByTestId("freelo-project-1"));
    expect(
      screen.getByRole("button", { name: /^dokončit$/i }),
    ).toBeEnabled();

    await user.click(screen.getByRole("button", { name: /^dokončit$/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("set_freelo_selected_projects", {
        connectionId: 1,
        projectIds: [1],
      }),
    );
  });
});

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

describe("Setup wizard", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("starts on step 1 (URL)", () => {
    renderSetup();
    expect(screen.getByLabelText(/jira base url/i)).toBeInTheDocument();
    expect(screen.getByText(/step 1 of 3/i)).toBeInTheDocument();
  });

  it("keeps Next disabled until URL is valid https://", async () => {
    const user = userEvent.setup();
    renderSetup();
    const input = screen.getByLabelText(/jira base url/i);
    const next = screen.getByRole("button", { name: /next/i });
    expect(next).toBeDisabled();

    await user.type(input, "http://nope");
    expect(next).toBeDisabled();
    expect(screen.getByText(/must start with https/i)).toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "https://acme.atlassian.net");
    expect(next).toBeEnabled();
  });

  it("advances through the steps with valid input", async () => {
    const user = userEvent.setup();
    renderSetup();

    // Step 1 → 2.
    await user.type(
      screen.getByLabelText(/jira base url/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));

    // Step 2.
    expect(screen.getByText(/step 2 of 3/i)).toBeInTheDocument();
    const emailInput = screen.getByLabelText(/account email/i);
    const next2 = screen.getByRole("button", { name: /next/i });
    expect(next2).toBeDisabled();
    await user.type(emailInput, "not-an-email");
    expect(next2).toBeDisabled();
    expect(screen.getByText(/must be a valid email/i)).toBeInTheDocument();
    await user.clear(emailInput);
    await user.type(emailInput, "alice@example.com");
    expect(next2).toBeEnabled();
    await user.click(next2);

    // Step 3.
    expect(screen.getByText(/step 3 of 3/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /test connection/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^finish$/i })).toBeDisabled();
  });

  it("enables Finish only after a successful connection test", async () => {
    const user = userEvent.setup();
    renderSetup();

    // Drive through to step 3.
    await user.type(
      screen.getByLabelText(/jira base url/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.type(
      screen.getByLabelText(/account email/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));

    // Test connection should be disabled until token meets min length.
    const testBtn = screen.getByRole("button", { name: /test connection/i });
    expect(testBtn).toBeDisabled();
    await user.type(screen.getByLabelText(/jira api token/i), "abcd1234567890");
    expect(testBtn).toBeEnabled();
    expect(screen.getByRole("button", { name: /^finish$/i })).toBeDisabled();

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
      expect(screen.getByText(/connected as alice example/i)).toBeInTheDocument(),
    );

    // Verify the wrapper translated the JS args into the snake_case shape.
    expect(mockInvoke).toHaveBeenCalledWith("test_jira_connection", {
      baseUrl: "https://acme.atlassian.net",
      email: "alice@example.com",
      token: "abcd1234567890",
    });
    expect(screen.getByRole("button", { name: /^finish$/i })).toBeEnabled();
  });

  it("calls save_config and enter_main_app on Finish, then navigates home", async () => {
    const user = userEvent.setup();
    renderSetup();

    await user.type(
      screen.getByLabelText(/jira base url/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.type(
      screen.getByLabelText(/account email/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));
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

    await user.click(screen.getByRole("button", { name: /test connection/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^finish$/i })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: /^finish$/i }));

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

    await user.type(
      screen.getByLabelText(/jira base url/i),
      "https://acme.atlassian.net",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.type(
      screen.getByLabelText(/account email/i),
      "alice@example.com",
    );
    await user.click(screen.getByRole("button", { name: /next/i }));
    await user.type(screen.getByLabelText(/jira api token/i), "abcd1234567890");

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_jira_connection") {
        throw "unauthorized";
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    await user.click(screen.getByRole("button", { name: /test connection/i }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(/unauthorized/i),
    );
    expect(screen.getByRole("button", { name: /^finish$/i })).toBeDisabled();
  });
});

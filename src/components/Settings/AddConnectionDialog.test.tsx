/**
 * Tests for the self-hosted / on-premise Jira toggle in the Add-connection
 * dialog.
 *
 * The backend already supports an `allow_custom_host` flag on the Jira
 * connection config (`JiraConnectionConfig::allow_custom_host`) which relaxes
 * the host allow-list from "*.atlassian.net only" to "any public https host".
 * Until now there was NO UI control to set it — on-prem users could only
 * enable it by hand-editing the config / importing a backup.
 *
 * These tests pin the UI contract:
 *   - Checking the toggle makes `add_connection` persist
 *     `config.allow_custom_host === true`.
 *   - The "Otestovat" (Test) button must route through
 *     `test_connection_for_provider` carrying the same flag, otherwise a
 *     self-hosted URL would fail the pre-save probe (backend rejects the host)
 *     even though the eventual Save would have been allowed.
 *   - Leaving it unchecked keeps the cloud-only behaviour (flag false).
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { vi } from "vitest";

import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import { AddConnectionDialog } from "./AddConnectionDialog";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

function renderDialog() {
  return render(
    <AddConnectionDialog open onClose={() => {}} onSaved={() => {}} />,
  );
}

async function gotoJiraCreds(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByTestId("provider-card-jira"));
  await user.click(screen.getByRole("button", { name: /^další$/i }));
}

async function fillJira(
  user: ReturnType<typeof userEvent.setup>,
  url = "https://jira.acme.local",
) {
  await user.type(screen.getByLabelText(/základní url jiry/i), url);
  await user.type(screen.getByLabelText(/e-mail atlassian účtu/i), "u@acme.local");
  await user.type(screen.getByLabelText(/jira api token/i), "tok_ABCDEF123456");
}

const OK_USER = {
  accountId: "acc-1",
  displayName: "Jan Novák",
  emailAddress: "u@acme.local",
  provider: "jira",
};

describe("AddConnectionDialog — self-hosted Jira toggle", () => {
  it("persists allow_custom_host=true on save when the toggle is checked", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_connection_for_provider") return OK_USER;
      if (cmd === "add_connection") {
        return {
          id: 9,
          provider: "jira",
          name: "Jira",
          enabled: true,
          created_at: 0,
          updated_at: 0,
          config: {},
          has_token: true,
        };
      }
      return null;
    });

    const user = userEvent.setup();
    renderDialog();
    await gotoJiraCreds(user);
    await fillJira(user);

    await user.click(screen.getByRole("checkbox", { name: /self-hosted/i }));
    await user.click(screen.getByTestId("add-conn-test"));
    await screen.findByRole("status"); // "Připojeno jako …"
    await user.click(screen.getByTestId("add-conn-save-jira"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("add_connection", {
        args: expect.objectContaining({
          provider: "jira",
          config: expect.objectContaining({ allow_custom_host: true }),
        }),
      });
    });
  });

  it("routes the Test probe through test_connection_for_provider with the flag", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_connection_for_provider") return OK_USER;
      return null;
    });

    const user = userEvent.setup();
    renderDialog();
    await gotoJiraCreds(user);
    await fillJira(user);
    await user.click(screen.getByRole("checkbox", { name: /self-hosted/i }));
    await user.click(screen.getByTestId("add-conn-test"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("test_connection_for_provider", {
        args: expect.objectContaining({
          provider: "jira",
          config: expect.objectContaining({ allow_custom_host: true }),
        }),
      });
    });
  });

  it("keeps allow_custom_host falsy when the toggle is left unchecked", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "test_connection_for_provider") return OK_USER;
      if (cmd === "add_connection") {
        return {
          id: 9,
          provider: "jira",
          name: "Jira",
          enabled: true,
          created_at: 0,
          updated_at: 0,
          config: {},
          has_token: true,
        };
      }
      return null;
    });

    const user = userEvent.setup();
    renderDialog();
    await gotoJiraCreds(user);
    await fillJira(user, "https://acme.atlassian.net");
    await user.click(screen.getByTestId("add-conn-test"));
    await screen.findByRole("status");
    await user.click(screen.getByTestId("add-conn-save-jira"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "add_connection",
        expect.anything(),
      );
    });
    const call = mockInvoke.mock.calls.find((c) => c[0] === "add_connection");
    const cfg = (call?.[1] as { args: { config: Record<string, unknown> } })
      .args.config;
    expect(cfg.allow_custom_host).toBeFalsy();
  });
});

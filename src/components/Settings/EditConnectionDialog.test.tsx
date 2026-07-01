/**
 * Tests for the self-hosted / on-premise Jira toggle in the Edit-connection
 * dialog. Mirrors the Add-dialog contract:
 *   - The toggle hydrates from the connection's stored
 *     `config.allow_custom_host`.
 *   - Toggling it on and saving persists the flag via `update_connection`.
 *   - The Test probe (shown when replacing the token) routes through
 *     `test_connection_for_provider` carrying the flag.
 */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ConnectionDto } from "../../api/types";
import { coreMock, mockInvoke } from "../../test/__mocks__/tauri";

import { EditConnectionDialog } from "./EditConnectionDialog";

vi.mock("@tauri-apps/api/core", () => coreMock);

beforeEach(() => {
  mockInvoke.mockReset();
});

function jiraConn(extraConfig: Record<string, unknown> = {}): ConnectionDto {
  return {
    id: 3,
    provider: "jira",
    name: "SAB",
    enabled: true,
    created_at: 0,
    updated_at: 0,
    config: {
      base_url: "https://jira.acme.local",
      email: "u@acme.local",
      ...extraConfig,
    },
    has_token: true,
  };
}

const OK_USER = {
  accountId: "acc-1",
  displayName: "Jan Novák",
  emailAddress: "u@acme.local",
  provider: "jira",
};

describe("EditConnectionDialog — self-hosted Jira toggle", () => {
  it("pre-checks the toggle from config.allow_custom_host", () => {
    render(
      <EditConnectionDialog
        open
        conn={jiraConn({ allow_custom_host: true })}
        onClose={() => {}}
        onSaved={() => {}}
      />,
    );
    expect(
      screen.getByRole("checkbox", { name: /self-hosted/i }),
    ).toBeChecked();
  });

  it("persists allow_custom_host via update_connection when toggled on", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "update_connection") {
        return { ...jiraConn({ allow_custom_host: true }) };
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <EditConnectionDialog
        open
        conn={jiraConn()}
        onClose={() => {}}
        onSaved={() => {}}
      />,
    );

    const cb = screen.getByRole("checkbox", { name: /self-hosted/i });
    expect(cb).not.toBeChecked();
    await user.click(cb);
    await user.click(screen.getByTestId("edit-conn-save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("update_connection", {
        args: expect.objectContaining({
          id: 3,
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
    render(
      <EditConnectionDialog
        open
        conn={jiraConn({ allow_custom_host: true })}
        onClose={() => {}}
        onSaved={() => {}}
      />,
    );

    await user.click(screen.getByTestId("edit-conn-replace-secret"));
    await user.type(
      screen.getByLabelText(/nový jira api token/i),
      "tok_ABCDEF123456",
    );
    await user.click(screen.getByRole("button", { name: /otestovat/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("test_connection_for_provider", {
        args: expect.objectContaining({
          provider: "jira",
          config: expect.objectContaining({ allow_custom_host: true }),
        }),
      });
    });
  });
});

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockInvoke, coreMock, eventMock } = vi.hoisted(() => {
  const invokeFn = vi.fn();
  return {
    mockInvoke: invokeFn,
    coreMock: { invoke: invokeFn },
    eventMock: {
      listen: vi.fn(async () => () => {}),
      emit: vi.fn(async () => {}),
      emitTo: vi.fn(async () => {}),
    },
  };
});

vi.mock("@tauri-apps/api/core", () => coreMock);
vi.mock("@tauri-apps/api/event", () => eventMock);

import { usePrefsStore } from "../stores/prefsStore";
import Settings from "./Settings";

function renderSettings() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Settings />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Settings route", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    eventMock.listen.mockClear();
    usePrefsStore.setState({
      dailyGoalSeconds: 8 * 60 * 60,
      hourlyRate: 0,
      currency: "CZK",
      widgetFormat: "HH:MM:SS",
      theme: "dark",
      fontSize: "md",
      density: "comfortable",
      hydrated: true,
      error: null,
    });
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_current_config")
        return { base_url: "https://acme.atlassian.net", email: "a@b.com" };
      if (cmd === "set_theme" || cmd === "set_font_size" || cmd === "set_density")
        return null;
      if (cmd === "sign_out") return null;
      return null;
    });
  });

  it("renders the tab strip with four tabs", async () => {
    renderSettings();
    expect(await screen.findByRole("tab", { name: /connection/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /appearance/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /time/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /about/i })).toBeInTheDocument();
  });

  it("connection tab shows current config", async () => {
    renderSettings();
    await waitFor(() =>
      expect(screen.getByText("https://acme.atlassian.net")).toBeInTheDocument(),
    );
    expect(screen.getByText("a@b.com")).toBeInTheDocument();
  });

  it("appearance tab has theme radio group", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.click(screen.getByRole("tab", { name: /appearance/i }));
    expect(
      await screen.findByRole("radiogroup", { name: /theme/i }),
    ).toBeInTheDocument();
  });

  it("theme toggle invokes set_theme", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.click(screen.getByRole("tab", { name: /appearance/i }));
    const lightOption = await screen.findByRole("radio", { name: /light/i });
    await user.click(lightOption);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_theme", { theme: "light" });
    });
  });

  it("about tab shows the version", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.click(screen.getByRole("tab", { name: /about/i }));
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
  });

  it("sign out flow uses ConfirmButton then invokes sign_out", async () => {
    const user = userEvent.setup();
    renderSettings();
    // Wait for config to load so the Sign out button is rendered.
    await screen.findByText("a@b.com");
    await user.click(screen.getByRole("button", { name: /^sign out$/i }));
    const confirm = await screen.findByRole("button", { name: /yes, sign out/i });
    await user.click(confirm);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("sign_out");
    });
  });
});

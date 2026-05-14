/**
 * Typed wrappers around the Tauri `invoke` IPC bridge.
 *
 * Centralising the command names and argument shapes here means the rest of
 * the React code can stay free of stringly-typed `invoke()` calls — and lets
 * tests mock at this thin layer instead of poking `@tauri-apps/api/core`.
 */
import { invoke } from "@tauri-apps/api/core";

import type { JiraConfig, JiraUser, SaveConfigArgs } from "./types";

/** `has_config(): bool` — true iff config + keychain token + Jira client all loaded. */
export function hasConfig(): Promise<boolean> {
  return invoke<boolean>("has_config");
}

/** `save_config(args): ()` — persists config to disk + token to OS keychain. */
export function saveConfig(config: JiraConfig, token: string): Promise<void> {
  const args: SaveConfigArgs = { config, token };
  return invoke<void>("save_config", { args });
}

/**
 * `test_jira_connection(base_url, email, token): JiraUser` — probe the supplied
 * credentials against `/rest/api/3/myself` without persisting anything.
 */
export function testJiraConnection(
  baseUrl: string,
  email: string,
  token: string,
): Promise<JiraUser> {
  return invoke<JiraUser>("test_jira_connection", {
    baseUrl,
    email,
    token,
  });
}

/** `enter_main_app(): ()` — backend emits `main-window:navigate` with `"main"`. */
export function enterMainApp(): Promise<void> {
  return invoke<void>("enter_main_app");
}

/** `enter_setup(): ()` — backend emits `main-window:navigate` with `"setup"`. */
export function enterSetup(): Promise<void> {
  return invoke<void>("enter_setup");
}

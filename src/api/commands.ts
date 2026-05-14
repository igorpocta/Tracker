/**
 * Typed wrappers around the Tauri `invoke` IPC bridge.
 *
 * Centralising the command names and argument shapes here means the rest of
 * the React code can stay free of stringly-typed `invoke()` calls — and lets
 * tests mock at this thin layer instead of poking `@tauri-apps/api/core`.
 */
import { invoke } from "@tauri-apps/api/core";

import type {
  ActiveTimerState,
  DensityPref,
  FontSizePref,
  IssueRow,
  JiraConfig,
  JiraUser,
  RefreshAllResult,
  SaveConfigArgs,
  ThemePref,
  VisibleTicket,
  WorklogRow,
} from "./types";

// -----------------------------------------------------------------------------
// Config / setup
// -----------------------------------------------------------------------------

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

/** `open_main_window(): ()` — focuses the main window. */
export function openMainWindow(): Promise<void> {
  return invoke<void>("open_main_window");
}

// -----------------------------------------------------------------------------
// Timer
// -----------------------------------------------------------------------------

/** `get_timer_state(): Option<ActiveTimerState>` — null when no timer is running. */
export function getTimerState(): Promise<ActiveTimerState | null> {
  return invoke<ActiveTimerState | null>("get_timer_state");
}

/** `start_timer(issue_key, started_at_ms?): ActiveTimerState` */
export function startTimer(
  issueKey: string,
  startedAtMs?: number,
): Promise<ActiveTimerState> {
  return invoke<ActiveTimerState>("start_timer", {
    issueKey,
    startedAtMs: startedAtMs ?? null,
  });
}

/**
 * `stop_timer_inner(comment?): Option<WorklogRow>` — stops the active timer,
 * pushes to Jira (if reachable), records locally. Null if no timer was running.
 */
export function stopTimer(comment?: string): Promise<WorklogRow | null> {
  return invoke<WorklogRow | null>("stop_timer_inner", {
    comment: comment ?? null,
  });
}

/** `update_timer_start(started_at_ms): ActiveTimerState` — adjust the start time. */
export function updateTimerStart(startedAtMs: number): Promise<ActiveTimerState> {
  return invoke<ActiveTimerState>("update_timer_start", {
    startedAtMs,
  });
}

// -----------------------------------------------------------------------------
// Issues
// -----------------------------------------------------------------------------

export function searchIssuesCache(
  query: string,
  limit?: number,
): Promise<IssueRow[]> {
  return invoke<IssueRow[]>("search_issues_cache", {
    query,
    limit: limit ?? null,
  });
}

export function getRecentIssues(limit?: number): Promise<IssueRow[]> {
  return invoke<IssueRow[]>("get_recent_issues", { limit: limit ?? null });
}

export function getSuggestedIssues(limit?: number): Promise<IssueRow[]> {
  return invoke<IssueRow[]>("get_suggested_issues", { limit: limit ?? null });
}

/** `refresh_cache(): number` — pulls latest issues from Jira; returns count. */
export function refreshCache(): Promise<number> {
  return invoke<number>("refresh_cache");
}

// -----------------------------------------------------------------------------
// Worklog history
// -----------------------------------------------------------------------------

export function getWorklogIssues(limit?: number): Promise<WorklogRow[]> {
  return invoke<WorklogRow[]>("get_worklog_issues", { limit: limit ?? null });
}

// -----------------------------------------------------------------------------
// Misc
// -----------------------------------------------------------------------------

export function openJiraIssue(key: string): Promise<void> {
  return invoke<void>("open_jira_issue", { key });
}

export function openUrl(url: string): Promise<void> {
  return invoke<void>("open_url", { url });
}

// -----------------------------------------------------------------------------
// Prefs
// -----------------------------------------------------------------------------

export function getDailyGoal(): Promise<number> {
  return invoke<number>("get_daily_goal");
}

export function setDailyGoal(seconds: number): Promise<void> {
  return invoke<void>("set_daily_goal", { seconds });
}

export function getHourlyRate(): Promise<number> {
  return invoke<number>("get_hourly_rate");
}

export function setHourlyRate(rate: number): Promise<void> {
  return invoke<void>("set_hourly_rate", { rate });
}

export function setWidgetFormat(format: string): Promise<void> {
  return invoke<void>("set_widget_format", { format });
}

export function setAppIcon(icon: string): Promise<void> {
  return invoke<void>("set_app_icon", { icon });
}

// -----------------------------------------------------------------------------
// Browser extension stubs (Phase 9)
// -----------------------------------------------------------------------------

export function getBrowserContext(): Promise<string | null> {
  return invoke<string | null>("get_browser_context");
}

export function getCurrentVisibleTicket(): Promise<VisibleTicket | null> {
  return invoke<VisibleTicket | null>("get_current_visible_ticket");
}

export function getExtensionLastHeartbeat(): Promise<number | null> {
  return invoke<number | null>("get_extension_last_heartbeat");
}

// -----------------------------------------------------------------------------
// Connection management (Phase 11A)
// -----------------------------------------------------------------------------

/** `get_current_config(): Option<JiraConfig>` — never includes token. */
export function getCurrentConfig(): Promise<JiraConfig | null> {
  return invoke<JiraConfig | null>("get_current_config");
}

/**
 * `update_config(new_cfg, new_token?): ()` — replace base_url/email and
 * optionally rotate the API token. Pass `null` for `newToken` to keep the
 * existing one.
 */
export function updateConfig(
  newConfig: JiraConfig,
  newToken?: string | null,
): Promise<void> {
  return invoke<void>("update_config", {
    newCfg: newConfig,
    newToken: newToken ?? null,
  });
}

/** `sign_out(): ()` — clears config file + keychain entry. */
export function signOut(): Promise<void> {
  return invoke<void>("sign_out");
}

// -----------------------------------------------------------------------------
// Worklog data (Phase 11A)
// -----------------------------------------------------------------------------

/**
 * `refresh_all(from_days): { issues, worklogs }` — pull latest issues and the
 * last `from_days` of worklogs from Jira.
 */
export function refreshAll(fromDays: number): Promise<RefreshAllResult> {
  return invoke<RefreshAllResult>("refresh_all", { fromDays });
}

/**
 * `get_worklogs_for_range(from_unix_s, to_unix_s, author?): WorklogRow[]`.
 * Author defaults to the configured email when null.
 */
export function getWorklogsForRange(
  fromUnixS: number,
  toUnixS: number,
  author?: string | null,
): Promise<WorklogRow[]> {
  return invoke<WorklogRow[]>("get_worklogs_for_range", {
    fromUnixS,
    toUnixS,
    author: author ?? null,
  });
}

// -----------------------------------------------------------------------------
// Appearance prefs (Phase 11A)
// -----------------------------------------------------------------------------

export function getTheme(): Promise<ThemePref> {
  return invoke<ThemePref>("get_theme");
}

export function setTheme(theme: ThemePref): Promise<void> {
  return invoke<void>("set_theme", { theme });
}

export function getFontSize(): Promise<FontSizePref> {
  return invoke<FontSizePref>("get_font_size");
}

export function setFontSize(size: FontSizePref): Promise<void> {
  return invoke<void>("set_font_size", { size });
}

export function getDensity(): Promise<DensityPref> {
  return invoke<DensityPref>("get_density");
}

export function setDensity(density: DensityPref): Promise<void> {
  return invoke<void>("set_density", { density });
}

// -----------------------------------------------------------------------------
// Phase 12: Accent color + currency
// -----------------------------------------------------------------------------

export function getAccentColor(): Promise<string> {
  return invoke<string>("get_accent_color");
}

export function setAccentColor(accent: string): Promise<void> {
  return invoke<void>("set_accent_color", { accent });
}

export function getCurrency(): Promise<string> {
  return invoke<string>("get_currency");
}

export function setCurrency(currency: string): Promise<void> {
  return invoke<void>("set_currency", { currency });
}

// -----------------------------------------------------------------------------
// Phase 13: palette mode (mono / dual)
// -----------------------------------------------------------------------------

export function getPaletteMode(): Promise<string> {
  return invoke<string>("get_palette_mode");
}

export function setPaletteMode(mode: string): Promise<void> {
  return invoke<void>("set_palette_mode", { mode });
}

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
  AuditEntry,
  AuditLogFilter,
  CacheStats,
  ConnectionDto,
  DensityPref,
  FontSizePref,
  FreeloProjectDto,
  IssueRow,
  JiraConfig,
  JiraUser,
  MoveWorklogResult,
  ProviderUser,
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

/**
 * `start_timer(issue_key?, started_at_ms?, comment?): ActiveTimerState`
 *
 * Phase 18A — Item 4: `issueKey` is optional. Passing `undefined` (or an
 * empty string) starts an "unassigned" timer — the UI surfaces a red ⚠ until
 * the user assigns an issue via `assignActiveTimer` (or stops, in which case
 * the resulting worklog has `pending_assignment = true`).
 *
 * Phase 18B — Item 6: optional `comment` attaches an in-flight note that is
 * carried into the eventual worklog (unless the StopDialog provides one).
 */
export function startTimer(
  issueKey?: string | null,
  startedAtMs?: number,
  comment?: string | null,
): Promise<ActiveTimerState> {
  return invoke<ActiveTimerState>("start_timer", {
    issueKey: issueKey ?? null,
    startedAtMs: startedAtMs ?? null,
    comment: comment ?? null,
  });
}

/**
 * `update_timer_comment(comment?): ActiveTimerState` — change the in-flight
 * comment on the running timer. Pass `null` (or an empty string) to clear it.
 */
export function updateTimerComment(
  comment: string | null,
): Promise<ActiveTimerState> {
  return invoke<ActiveTimerState>("update_timer_comment", {
    comment: comment ?? null,
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

/**
 * `discard_timer(): bool` — completely cancels the running timer without
 * creating any worklog. Returns `true` if a timer was cleared, `false` if
 * none was running. Emits `timer-discarded` + `timer-stopped` so all
 * surfaces (popover, tray) refresh.
 */
export function discardTimer(): Promise<boolean> {
  return invoke<boolean>("discard_timer");
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

/** `get_cache_stats(): CacheStats` — counts of cached issues + worklogs. */
export function getCacheStats(): Promise<CacheStats> {
  return invoke<CacheStats>("get_cache_stats");
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

/**
 * Provider-aware "open this issue in the user's default browser" command.
 *
 * Routes by the synthetic key prefix:
 *   - `FREELO-{id}`   → https://app.freelo.io/task/{id}
 *   - `FREELO-P-{id}` → https://app.freelo.io/project/{id}
 *   - anything else   → looks up the issue's owning connection in the cache
 *                       and opens `<that connection's base_url>/browse/{key}`.
 */
export function openIssue(key: string): Promise<void> {
  return invoke<void>("open_issue", { key });
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
// Worklog mutations (Phase 15)
// -----------------------------------------------------------------------------

/** Create a manual worklog (AddEntry panel). Pushes to Jira, then caches. */
export function createManualWorklog(args: {
  issueKey: string;
  startedAtMs: number;
  durationSeconds: number;
  comment?: string | null;
}): Promise<WorklogRow> {
  return invoke<WorklogRow>("create_manual_worklog", {
    issueKey: args.issueKey,
    startedAtMs: args.startedAtMs,
    durationSeconds: args.durationSeconds,
    comment: args.comment ?? null,
  });
}

/**
 * Update an existing worklog. `null` (or `undefined`) values leave the field
 * unchanged. Returns the updated row.
 */
export function updateWorklog(args: {
  worklogId: string;
  issueKey: string;
  newStartedAtMs?: number | null;
  newDurationSeconds?: number | null;
  newComment?: string | null;
}): Promise<WorklogRow> {
  return invoke<WorklogRow>("update_worklog", {
    worklogId: args.worklogId,
    issueKey: args.issueKey,
    newStartedAtMs: args.newStartedAtMs ?? null,
    newDurationSeconds: args.newDurationSeconds ?? null,
    newComment: args.newComment ?? null,
  });
}

/**
 * Soft-delete a worklog. Marks `pending_delete_at` locally; backend fires
 * the actual Jira DELETE after a 5s undo grace window. Frontend should
 * optimistically hide the row and show an undo toast.
 */
export function deleteWorklog(
  worklogId: string,
  issueKey: string,
): Promise<void> {
  return invoke<void>("delete_worklog", { worklogId, issueKey });
}

/** Cancel a pending delete within the 5s grace window. */
export function undoDeleteWorklog(worklogId: string): Promise<void> {
  return invoke<void>("undo_delete_worklog", { worklogId });
}

/**
 * Move a worklog from one issue to another. Backed by POST new + DELETE old.
 *
 * On the partial-success path (POST succeeded, DELETE failed) the backend
 * returns an error string starting with "Original worklog still exists on …"
 * so the UI can offer a manual retry.
 */
export function moveWorklog(args: {
  oldIssueKey: string;
  oldWorklogId: string;
  newIssueKey: string;
  startedAtMs: number;
  durationSeconds: number;
  comment?: string | null;
}): Promise<MoveWorklogResult> {
  return invoke<MoveWorklogResult>("move_worklog", {
    oldIssueKey: args.oldIssueKey,
    oldWorklogId: args.oldWorklogId,
    newIssueKey: args.newIssueKey,
    startedAtMs: args.startedAtMs,
    durationSeconds: args.durationSeconds,
    comment: args.comment ?? null,
  });
}

/**
 * Read audit entries newest-first. Supports pagination (`beforeId`) and
 * filtering (`ops` to restrict to specific operations, `onlyFailed` for
 * troubleshooting). Defaults: limit 50, no filter.
 */
export function getAuditLog(filter?: AuditLogFilter): Promise<AuditEntry[]> {
  return invoke<AuditEntry[]>("get_audit_log", {
    limit: filter?.limit ?? null,
    beforeId: filter?.beforeId ?? null,
    ops: filter?.ops ?? null,
    onlyFailed: filter?.onlyFailed ?? null,
  });
}

/**
 * Phase 16 — re-create a worklog deleted in `delete` / `sync_tombstone` audit
 * entries. Returns the new `WorklogRow` (with a fresh `jira_worklog_id`).
 *
 * Note: Jira does not support resurrecting by id — this POSTs a brand-new
 * worklog. The original deleted id stays gone; the audit log preserves the
 * linkage via `source_audit_id`.
 */
export function restoreDeletedWorklog(auditId: number): Promise<WorklogRow> {
  return invoke<WorklogRow>("restore_deleted_worklog", { auditId });
}

/**
 * Phase 16 — push an `update` audit's `before_json` snapshot back to Jira as
 * a fresh PUT, effectively reverting the change. Errors if the worklog has
 * been deleted in Jira since the original update.
 */
export function revertWorklogUpdate(auditId: number): Promise<WorklogRow> {
  return invoke<WorklogRow>("revert_worklog_update", { auditId });
}

/**
 * Phase 16 — replay a failed audit action with its captured arguments. The
 * exact request depends on the original op (create / update / delete).
 * Returns a small JSON blob describing the outcome.
 */
export function retryFailedAuditAction(
  auditId: number,
): Promise<{ op?: string; worklog_id?: string }> {
  return invoke<{ op?: string; worklog_id?: string }>(
    "retry_failed_audit_action",
    { auditId },
  );
}

/** Hard-delete audit entries older than `olderThanDays` days. Returns count. */
export function purgeAuditLog(olderThanDays: number): Promise<number> {
  return invoke<number>("purge_audit_log", { olderThanDays });
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

// -----------------------------------------------------------------------------
// Phase 14: day timeline visibility
// -----------------------------------------------------------------------------

export function getDayTimelineVisible(): Promise<boolean> {
  return invoke<boolean>("get_day_timeline_visible");
}

export function setDayTimelineVisible(visible: boolean): Promise<void> {
  return invoke<void>("set_day_timeline_visible", { visible });
}

// -----------------------------------------------------------------------------
// Phase 18B — Item 22: earnings visibility
// -----------------------------------------------------------------------------

export function getEarningsVisible(): Promise<boolean> {
  return invoke<boolean>("get_earnings_visible");
}

export function setEarningsVisible(visible: boolean): Promise<void> {
  return invoke<void>("set_earnings_visible", { visible });
}

// -----------------------------------------------------------------------------
// Phase 18B — Item 26: favorite issues
// -----------------------------------------------------------------------------

export function listFavorites(): Promise<IssueRow[]> {
  return invoke<IssueRow[]>("list_favorites");
}

export function addFavorite(
  issueKey: string,
  connectionId?: number | null,
): Promise<void> {
  return invoke<void>("add_favorite", {
    issueKey,
    connectionId: connectionId ?? null,
  });
}

export function removeFavorite(issueKey: string): Promise<void> {
  return invoke<void>("remove_favorite", { issueKey });
}

export function isFavorite(issueKey: string): Promise<boolean> {
  return invoke<boolean>("is_favorite", { issueKey });
}

// -----------------------------------------------------------------------------
// Phase 18A — Multi-connection / multi-provider
// -----------------------------------------------------------------------------

// Re-export so existing imports of `ConnectionDto` from `./commands` keep
// working. Canonical definition lives in `./types`.
export type { ConnectionDto } from "./types";

export function listConnections(): Promise<ConnectionDto[]> {
  return invoke<ConnectionDto[]>("list_connections");
}

export function addConnection(args: {
  provider: string;
  name: string;
  config: Record<string, unknown>;
  token: string;
}): Promise<ConnectionDto> {
  return invoke<ConnectionDto>("add_connection", { args });
}

export function updateConnectionApi(args: {
  id: number;
  name?: string;
  config?: Record<string, unknown>;
  token?: string;
  enabled?: boolean;
}): Promise<ConnectionDto> {
  return invoke<ConnectionDto>("update_connection", { args });
}

export function removeConnection(id: number): Promise<void> {
  return invoke<void>("remove_connection", { id });
}

export function enableConnection(id: number, enabled: boolean): Promise<ConnectionDto> {
  return invoke<ConnectionDto>("enable_connection", { id, enabled });
}

export function testConnectionForProvider(args: {
  provider: string;
  config: Record<string, unknown>;
  token: string;
}): Promise<ProviderUser> {
  return invoke<ProviderUser>("test_connection_for_provider", { args });
}

// -----------------------------------------------------------------------------
// Phase 18E — Freelo project picker
// -----------------------------------------------------------------------------

export function listFreeloProjects(
  connectionId: number,
): Promise<FreeloProjectDto[]> {
  return invoke<FreeloProjectDto[]>("list_freelo_projects", { connectionId });
}

export function setFreeloSelectedProjects(
  connectionId: number,
  projectIds: number[],
): Promise<void> {
  return invoke<void>("set_freelo_selected_projects", {
    connectionId,
    projectIds,
  });
}

export function getFreeloSelectedProjects(
  connectionId: number,
): Promise<number[]> {
  return invoke<number[]>("get_freelo_selected_projects", { connectionId });
}

export function syncFreeloNow(connectionId: number): Promise<number> {
  return invoke<number>("sync_freelo_now", { connectionId });
}

export function listMyIssues(
  connectionId: number,
  limit?: number,
): Promise<IssueRow[]> {
  return invoke<IssueRow[]>("list_my_issues", {
    connectionId,
    limit: limit ?? null,
  });
}

// -----------------------------------------------------------------------------
// Phase 18A — Timer extensions (assign / unassigned)
// -----------------------------------------------------------------------------

/**
 * Assign an issue key to the currently running (unassigned) timer. Does NOT
 * push a worklog — the timer keeps running with the new issue attached.
 */
export function assignActiveTimer(issueKey: string): Promise<ActiveTimerState> {
  return invoke<ActiveTimerState>("assign_active_timer", { issueKey });
}

/**
 * Phase 18A — Item 4: assign an issue to a previously-stopped unassigned
 * worklog. POSTs a fresh worklog to Jira and links its id locally.
 */
export function assignWorklogIssue(
  worklogId: number,
  issueKey: string,
): Promise<WorklogRow> {
  return invoke<WorklogRow>("assign_worklog_issue", { worklogId, issueKey });
}

/**
 * Phase 18A — Item 7: hard-delete a worklog that exists only locally (no
 * `jira_worklog_id`). Used for pending-assignment rows and ones that failed
 * to sync to Jira (e.g. <60s rejections).
 */
export function deleteLocalOnlyWorklog(worklogId: number): Promise<void> {
  return invoke<void>("delete_local_only_worklog", { worklogId });
}

// -----------------------------------------------------------------------------
// Phase 18A — Rounding (Item 27)
// -----------------------------------------------------------------------------

export type RoundingMode = "none" | "up" | "down";

export function getRoundingMode(): Promise<RoundingMode> {
  return invoke<RoundingMode>("get_rounding_mode");
}

export function setRoundingMode(mode: RoundingMode): Promise<void> {
  return invoke<void>("set_rounding_mode", { mode });
}

export function getRoundingIntervalMinutes(): Promise<number> {
  return invoke<number>("get_rounding_interval_minutes");
}

export function setRoundingIntervalMinutes(minutes: number): Promise<void> {
  return invoke<void>("set_rounding_interval_minutes", { minutes });
}

// -----------------------------------------------------------------------------
// Phase 18A — Calendar (Item 2)
// -----------------------------------------------------------------------------

export interface NonWorkingDay {
  date: string;
  reason: string;
  label: string | null;
  created_at: number;
}

export function getWorkingWeekMask(): Promise<number> {
  return invoke<number>("get_working_week_mask");
}

export function setWorkingWeekMask(mask: number): Promise<void> {
  return invoke<void>("set_working_week_mask", { mask });
}

export function listNonWorkingDays(
  fromDate: string,
  toDate: string,
): Promise<NonWorkingDay[]> {
  return invoke<NonWorkingDay[]>("list_non_working_days", { fromDate, toDate });
}

export function addNonWorkingDay(
  date: string,
  reason: string,
  label?: string,
): Promise<void> {
  return invoke<void>("add_non_working_day", {
    date,
    reason,
    label: label ?? null,
  });
}

export function removeNonWorkingDay(date: string): Promise<void> {
  return invoke<void>("remove_non_working_day", { date });
}

export function isWorkingDay(date: string): Promise<boolean> {
  return invoke<boolean>("is_working_day", { date });
}

// -----------------------------------------------------------------------------
// Phase 18A — Activity (Item 32)
// -----------------------------------------------------------------------------

export interface DailyActivityRow {
  date: string;
  active_seconds: number;
  inactive_seconds: number;
  updated_at: number;
}

export interface ActivityIngestResult {
  active_added: number;
  inactive_added: number;
}

export function recordUserActivity(
  timestampMs: number,
): Promise<ActivityIngestResult> {
  return invoke<ActivityIngestResult>("record_user_activity", {
    timestampMs,
  });
}

export function getDailyActivity(date: string): Promise<DailyActivityRow> {
  return invoke<DailyActivityRow>("get_daily_activity", { date });
}

export function getActivityThresholdMin(): Promise<number> {
  return invoke<number>("get_activity_threshold_min");
}

export function setActivityThresholdMin(min: number): Promise<void> {
  return invoke<void>("set_activity_threshold_min", { min });
}

// -----------------------------------------------------------------------------
// Phase 18A — Autostart (Item 30)
//
// Thin wrappers around `tauri-plugin-autostart`. We talk to the plugin
// directly via its IPC namespace (`plugin:autostart|...`) so we don't have to
// add the JS package — the dependency is purely on the Rust side.
// -----------------------------------------------------------------------------

export function getAutostart(): Promise<boolean> {
  return invoke<boolean>("plugin:autostart|is_enabled");
}

export async function setAutostart(enabled: boolean): Promise<void> {
  if (enabled) {
    await invoke<void>("plugin:autostart|enable");
  } else {
    await invoke<void>("plugin:autostart|disable");
  }
}

// -----------------------------------------------------------------------------
// Phase 19 — Sentry opt-in
// -----------------------------------------------------------------------------

/**
 * `get_install_id(): String` — stable anonymous UUID generated once per
 * install. Used as Sentry's `user.id`; safe to log / display.
 */
export function getInstallId(): Promise<string> {
  return invoke<string>("get_install_id");
}

/** `get_sentry_enabled(): bool` — current opt-in state (default `false`). */
export function getSentryEnabled(): Promise<boolean> {
  return invoke<boolean>("get_sentry_enabled");
}

/**
 * `set_sentry_enabled(value): ()` — persist the new opt-in state. Backend
 * opportunistically re-initialises its SDK when the user toggles ON. The
 * caller is responsible for calling `initSentry` / `shutdownSentry` on the
 * frontend half to make the change take effect immediately.
 */
export function setSentryEnabled(value: boolean): Promise<void> {
  return invoke<void>("set_sentry_enabled", { value });
}

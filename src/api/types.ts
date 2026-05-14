/**
 * TypeScript shapes mirroring the Rust structs exposed through Tauri commands.
 *
 * Field names use snake_case where the Rust side does (we don't add custom
 * `#[serde(rename_all)]`) so the wrappers don't need to translate.
 */

/** Mirrors `src-tauri/src/config.rs::JiraConfig`. */
export interface JiraConfig {
  /** e.g. `https://acme.atlassian.net` (no trailing slash). */
  base_url: string;
  /** Atlassian account email used for Basic auth. */
  email: string;
}

/** Mirrors `src-tauri/src/jira/models.rs::JiraUser` (with serde renames). */
export interface JiraUser {
  accountId: string;
  displayName: string;
  emailAddress?: string | null;
}

/** Payload shape for the `save_config` command (matches `SaveConfigArgs`). */
export interface SaveConfigArgs {
  config: JiraConfig;
  token: string;
}

/** Payload emitted by the backend on `main-window:navigate`. */
export type NavigateTarget = "setup" | "main";

/** Mirrors `src-tauri/src/cache/issues.rs::IssueRow`. */
export interface IssueRow {
  issue_key: string;
  issue_id?: string | null;
  summary: string;
  status_category?: string | null;
  priority_order?: number | null;
  assignee_email?: string | null;
  assignee_account_id?: string | null;
  parent_key?: string | null;
  parent_summary?: string | null;
  issue_type?: string | null;
  time_spent?: number | null;
  aggregate_time_spent?: number | null;
  time_original_estimate?: number | null;
  time_estimate?: number | null;
  epic_key?: string | null;
  epic_summary?: string | null;
  updated_at: number;
}

/** Mirrors `src-tauri/src/cache/worklogs.rs::WorklogRow`. */
export interface WorklogRow {
  id?: number | null;
  issue_key: string;
  issue_id?: string | null;
  summary?: string | null;
  duration_s: number;
  /** Seconds since Unix epoch. */
  started_at: number;
  /** Seconds since Unix epoch. */
  logged_at: number;
  comment?: string | null;
  jira_worklog_id?: string | null;
}

/** Mirrors `src-tauri/src/commands/timer.rs::ActiveTimerState`. */
export interface ActiveTimerState {
  issue_key: string;
  /** Milliseconds since Unix epoch. */
  started_at: number;
  /** Elapsed seconds at the moment the snapshot was taken. */
  elapsed_seconds: number;
}

/** Visible Jira ticket reported by the browser extension. */
export interface VisibleTicket {
  issue_key: string;
  summary?: string | null;
  url?: string | null;
  seen_at?: number | null;
}

// -----------------------------------------------------------------------------
// Phase 11A backend additions
// -----------------------------------------------------------------------------

/** Theme preference. `auto` honors `prefers-color-scheme`. */
export type ThemePref = "auto" | "light" | "dark";

/** Font-size preference. */
export type FontSizePref = "sm" | "md" | "lg";

/** Density preference. */
export type DensityPref = "compact" | "comfortable";

/** Accent color identifier (maps to an HSL hue in the frontend). */
export type AccentColor =
  | "blue"
  | "indigo"
  | "violet"
  | "pink"
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "teal"
  | "graphite";

/** Supported currency codes. */
export type Currency = "CZK" | "EUR" | "USD" | "GBP" | "PLN" | "CHF";

/** Result of `refresh_all` — counts of records pulled from Jira. */
export interface RefreshAllResult {
  issues: number;
  worklogs: number;
}

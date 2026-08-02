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

/**
 * Unified provider-user payload returned by `test_connection_for_provider`.
 * Tracker uses this for both Jira and Freelo so the UI can show a uniform
 * "Connected as …" without provider-specific branches.
 */
export interface ProviderUser {
  accountId: string;
  displayName: string;
  emailAddress?: string | null;
  /** `"jira"` or `"freelo"`. */
  provider: string;
}

/** Provider kinds supported by Tracker. */
export type ProviderKind = "jira" | "freelo" | "toggl" | "clockify";

/**
 * Mirrors `src-tauri/src/commands/connections.rs::ConnectionDto`. Used by
 * `listConnections` and the multi-connection Settings UI.
 */
export interface ConnectionDto {
  id: number;
  provider: string;
  name: string;
  enabled: boolean;
  created_at: number;
  updated_at: number;
  config: Record<string, unknown>;
  has_token: boolean;
}

/**
 * Freelo project DTO returned by `list_freelo_projects` (mirrors
 * `src-tauri/src/commands/freelo.rs::FreeloProjectDto`).
 */
export interface FreeloProjectDto {
  id: number;
  name: string;
  state: string;
  /** Pre-filled from the persisted `config.selected_project_ids`. */
  selected: boolean;
}

/** Payload shape for the `save_config` command (matches `SaveConfigArgs`). */
export interface SaveConfigArgs {
  config: JiraConfig;
  token: string;
}

/** Payload emitted by the backend on `main-window:navigate`. */
export type NavigateTarget = "setup" | "main" | "settings";

/** Mirrors `src-tauri/src/cache/issues.rs::IssueRow`. */
export interface IssueRow {
  issue_key: string;
  /** Owning connection (tenant). Present since the multi-connection work;
   * used to disambiguate the same issue key across tenants. */
  connection_id?: number | null;
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

/**
 * Mirrors `src-tauri/src/cache/worklogs.rs::WorklogRow` after migration 0012.
 *
 * Backend vystaví nová pole (`connection_id`, `description`, `ended_at`,
 * `is_synced`, `synced_at`, `remote_id`, `summary` from JOIN) i legacy
 * aliasy (`comment`, `jira_worklog_id`, `source`, `pending_assignment`,
 * `duration_s`) pro zpětnou kompatibilitu FE.
 */
export interface WorklogRow {
  id?: number | null;
  /** ID připojení (Jira/Freelo), null pro lokálně vytvořený nesyncovaný řádek. */
  connection_id?: number | null;
  /** `null` pro řádky bez přiřazeného úkolu (timer-stop bez výběru). */
  issue_key?: string | null;
  description?: string | null;
  /** Derived: `ended_at - started_at`. Backend ho dál vystaví. */
  duration_s: number;
  /** Seconds since Unix epoch. */
  started_at: number;
  /** Seconds since Unix epoch. */
  ended_at?: number;
  /** Seconds since Unix epoch — kdy se řádek poprvé objevil lokálně. */
  logged_at: number;
  /** Last local UPDATE timestamp. */
  updated_at?: number;
  /** True když je řádek odeslaný do providera. */
  is_synced?: boolean;
  /** Last successful sync timestamp. */
  synced_at?: number | null;
  /** Provider's worklog id (bez prefixu). */
  remote_id?: string | null;
  /** Task title z join s `issues_v2`. */
  summary?: string | null;

  // ----- Legacy alias fields (still emitted by backend) -----
  comment?: string | null;
  jira_worklog_id?: string | null;
  source?: string | null;
  pending_assignment?: boolean;
  /** No longer populated post-0012, kept optional for old payloads. */
  author_account_id?: string | null;
  updated_at_jira?: number | null;
  /** Phase 15 — soft-delete + tombstone columns. */
  pending_delete_at?: number | null;
  tombstoned_at?: number | null;
}

/** Mirrors `src-tauri/src/commands/worklog.rs::MoveWorklogResultDto`. */
export interface MoveWorklogResult {
  new_worklog_id: string;
  new_row: WorklogRow;
  /** True if the move's DELETE half failed (rare). */
  original_still_exists: boolean;
}

/** Mirrors `src-tauri/src/cache/audit.rs::AuditEntry`. */
export interface AuditEntry {
  id: number;
  occurred_at: number;
  op: string;
  issue_key?: string | null;
  worklog_id?: string | null;
  before_json?: string | null;
  after_json?: string | null;
  success: boolean;
  error?: string | null;
  /** Phase 16 — id of the audit row that triggered this entry (restore/revert/retry). */
  source_audit_id?: number | null;
}

/** Discriminated set of op kinds we recognize. Unknown strings fall through. */
export type AuditOp =
  | "create"
  | "update"
  | "delete"
  | "move"
  | "sync_tombstone"
  | "undo"
  | "restore"
  | "revert"
  | "retry";

/** Filter args accepted by `getAuditLog`. All fields are optional. */
export interface AuditLogFilter {
  limit?: number;
  beforeId?: number | null;
  ops?: string[] | null;
  onlyFailed?: boolean | null;
}

/** Mirrors `src-tauri/src/commands/timer.rs::ActiveTimerState`. */
export interface ActiveTimerState {
  issue_key: string;
  /** Milliseconds since Unix epoch. */
  started_at: number;
  /** Elapsed seconds at the moment the snapshot was taken. */
  elapsed_seconds: number;
  /** Phase 18B — Item 6: in-flight comment attached to the running timer. */
  comment?: string | null;
  /** Issue title joined from the local issues cache for display. */
  summary?: string | null;
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

/**
 * Accent palette identifier.
 *
 * The original "Apple-style" hues (blue/indigo/…) from Phase 11 are still
 * accepted by the backend for backwards compatibility, but the UI now picks
 * from the Mono + Dual palette set inspired by the Tracker reference. See
 * `src/lib/accent.ts` for the canonical list of palette specs.
 */
export type AccentColor =
  // Legacy hues (still accepted by the backend)
  | "blue"
  | "indigo"
  | "violet"
  | "pink"
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "teal"
  | "graphite"
  // Mono palettes (Phase 13)
  | "aurora"
  | "trcker"
  | "love"
  | "halloween"
  // Mono palettes (Phase 18B — Item 16)
  | "mocha"
  | "electric"
  | "forest"
  | "plum"
  | "rust"
  // Dual palettes (Phase 13)
  | "czech"
  | "aurora-boreal"
  | "sakura-night"
  | "cyber-lime"
  | "nordic-fjord"
  // Dual palettes (2026 additions)
  | "tokyo-night"
  | "sunset-drive"
  | "deep-ocean"
  | "royal-velvet"
  | "forest-fire"
  // 2026 trend MONO palettes
  | "future-dusk"
  | "digital-lavender"
  | "butter-yellow"
  | "cherry-red"
  | "matcha"
  // 2026 trend DUAL palettes
  | "dusk-ember"
  | "matcha-clay"
  | "lavender-mint"
  | "butter-berry"
  | "slate-coral";

/**
 * Color palette mode — Mono (single primary) or Dual (primary + secondary).
 * Defaults to "mono" with an "aurora" accent.
 */
export type PaletteMode = "mono" | "dual";

/** Supported currency codes. */
export type Currency = "CZK" | "EUR" | "USD" | "GBP" | "PLN" | "CHF";

/** Outcome of one connection's sync (P2-2). */
export type SyncRunStatus = "success" | "partial" | "failed";

/** Result of `refresh_all` — counts of records pulled from Jira. */
export interface RefreshAllResult {
  issues: number;
  worklogs: number;
  /** P2-2: per-connection outcome counts + aggregate status. */
  succeeded: number;
  partial: number;
  failed: number;
  status: SyncRunStatus;
}

/** Result of `get_cache_stats` — counts of records currently in the local cache. */
export interface CacheStats {
  issues: number;
  worklogs_local: number;
}

// -----------------------------------------------------------------------------
// Focus mode
// -----------------------------------------------------------------------------

/** `app` rules match a bundle id / executable, `site` rules match a domain. */
export type FocusRuleKind = "app" | "site";
/** `allow` always wins over `block` — that's how exceptions are expressed. */
export type FocusRuleMode = "block" | "allow";
/** What happens to a blocked app. Ignored for `site` rules. */
export type FocusRuleAction = "hide" | "kill";

export interface FocusRule {
  id: number;
  kind: FocusRuleKind;
  mode: FocusRuleMode;
  /** Bundle id / executable name, or `domain[/path-prefix]`. */
  pattern: string;
  label: string | null;
  action: FocusRuleAction;
  enabled: boolean;
  created_at: number;
}

export interface FocusSettings {
  /** Allow-list mode for apps: block everything not explicitly allowed. */
  strict_apps: boolean;
  /** Allow-list mode for websites. */
  strict_sites: boolean;
  block_notifications: boolean;
  /** macOS Shortcut run when a session starts. */
  shortcut_on: string | null;
  /** macOS Shortcut run when a session ends. */
  shortcut_off: string | null;
  /** Minutes pre-filled in the start control. `0` = open-ended. */
  default_duration_min: number;
}

/** Session state plus the settings, flattened by the backend into one object. */
export interface FocusState extends FocusSettings {
  active: boolean;
  /** Unix seconds. */
  started_at: number | null;
  /** Unix seconds, `null` for an open-ended session. */
  ends_at: number | null;
  /** Bumped on every session or rule change. */
  generation: number;
}

/** One entry in the "pick an app to block" list. */
export interface RunningApp {
  /** The value that should become the rule pattern. */
  pattern: string;
  name: string;
  pid: number;
  /** Safe-listed apps can't be blocked — the UI explains why. */
  protected: boolean;
}

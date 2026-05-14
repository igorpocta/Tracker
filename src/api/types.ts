/**
 * TypeScript shapes mirroring the Rust structs exposed through Tauri commands.
 *
 * Only the slice consumed by the Setup wizard is modelled here; later phases
 * will extend this file as more commands are wired up.
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

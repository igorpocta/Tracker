//! Worklog history, mutation, sync, and audit/reconstruction commands.
//!
//! This module owns the full Phase-15 mutation surface plus the surrounding
//! sync orchestration and Phase-16 audit reconstruction wrappers. It is
//! split into three buckets for readability:
//!
//! - [`sync`]: cross-connection sync orchestration (`refresh_all`,
//!   `refresh_connection`, `sync_one_connection`, sync-error/run history).
//! - [`crud`]: per-worklog CRUD + lifecycle (`create_manual_worklog`,
//!   `update_worklog`, `delete_worklog`, `move_worklog`, `split_worklog`,
//!   `push_local_worklog`, `assign_worklog_issue`, the background
//!   `commit_pending_delete` worker, and the local-only helpers).
//! - [`audit`]: audit log queries + Phase-16 reconstruction commands
//!   (`get_audit_log`, `purge_audit_log`, `restore_deleted_worklog`,
//!   `revert_worklog_update`, `retry_failed_audit_action`).
//!
//! Every `#[tauri::command]` function and the cross-module helpers
//! (`sync_one_connection`, `commit_pending_delete`, `SyncMode`,
//! `RefreshAllResult`, …) are re-exported here so `lib.rs::generate_handler!`
//! and other callers continue to address them via the `commands::worklog::`
//! path.

pub mod audit;
pub mod crud;
pub mod flush;
pub mod sync;

pub use audit::*;
pub use crud::*;
pub use flush::flush_unsynced_worklogs;
pub use sync::*;

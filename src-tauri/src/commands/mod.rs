//! Tauri command surface exposed to the React frontend via the IPC bridge.
//!
//! Each submodule groups commands by concern (config, timer, issues, …) and
//! intentionally splits each command into two parts:
//!
//! 1. A `*_inner` (or similarly suffixed) plain function operating on the
//!    underlying primitives (`Db`, `JiraClient`, …). These are easily unit
//!    testable from `tests/`.
//! 2. A thin `#[tauri::command]` wrapper that pulls the right pieces out of
//!    [`crate::state::AppState`] and forwards to the inner function, mapping
//!    any error to `String` (Tauri requires the error type to be `Serialize`).

pub mod activity;
pub mod backup;
pub mod browser;
pub mod calendar;
pub mod config;
pub mod connections;
pub mod dashboard;
pub mod favorites;
pub mod freelo;
pub mod issues;
pub mod misc;
pub mod prefs;
pub mod rounding;
pub mod sentry;
pub mod streaks;
pub mod suggestions;
pub mod system_idle;
pub mod timer;
pub mod tray;
pub mod worklog;

pub use browser::{get_browser_context, get_current_visible_ticket, get_extension_last_heartbeat};
pub use config::{
    enter_main_app, enter_setup, get_current_config, has_config, open_main_window, save_config,
    sign_out, test_jira_connection, update_config,
};
pub use issues::{get_recent_issues, get_suggested_issues, refresh_cache, search_issues_cache};
pub use misc::{haptic_feedback, open_issue, open_jira_issue, open_url};
pub use prefs::{
    get_daily_goal, get_density, get_font_size, get_hourly_rate, get_theme, set_app_icon,
    set_daily_goal, set_density, set_font_size, set_hourly_rate, set_theme, set_widget_format,
};
pub use timer::{
    discard_timer, get_timer_state, start_timer, stop_timer_inner, update_timer_start,
};
pub use tray::{
    hide_tray_popover, quit_app, set_tray_available, show_tray_popover, toggle_tray_popover,
};
pub use worklog::{
    create_manual_worklog, delete_worklog, get_audit_log, get_sync_errors, get_worklog_issues,
    get_worklogs_for_range, move_worklog, push_local_worklog, refresh_all, refresh_connection,
    undo_delete_worklog, update_local_worklog, update_worklog, MoveWorklogResultDto,
    RefreshAllResult,
};

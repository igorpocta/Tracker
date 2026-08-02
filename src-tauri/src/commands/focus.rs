//! Tauri commands for Focus mode.
//!
//! Thin wrappers over [`crate::focus::engine`] plus the rule CRUD. Validation
//! lives here rather than in the store so the frontend gets a readable message
//! instead of a SQL constraint error.

use chrono::Utc;

use crate::cache::{self, focus::FocusRuleRow};
use crate::focus::engine::{self, FocusSettings, FocusStateDto};
use crate::focus::overlay::OverlayNotice;
use crate::focus::{apps, notify, rules};
use crate::state::AppState;

const KINDS: &[&str] = &["app", "site"];
const MODES: &[&str] = &["block", "allow"];
const ACTIONS: &[&str] = &["hide", "kill"];

/// One entry in the "pick an app to block" list.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningAppDto {
    /// The value that should become the rule pattern — a bundle id on macOS,
    /// an executable name on Windows.
    pub pattern: String,
    pub name: String,
    pub pid: i64,
    /// `true` when the app is safe-listed and therefore cannot be blocked.
    pub protected: bool,
}

fn require_one_of(value: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let lowered = value.trim().to_ascii_lowercase();
    if allowed.contains(&lowered.as_str()) {
        Ok(lowered)
    } else {
        Err(format!(
            "Neplatná hodnota pole {field}: „{value}\". Povoleno: {}.",
            allowed.join(", ")
        ))
    }
}

/// Validate and canonicalise a rule before it hits the database.
///
/// Site patterns are normalised (`https://www.Reddit.com/` → `reddit.com`) so
/// two spellings of the same rule collapse onto one row instead of both
/// sitting in the list doing the same job.
pub fn normalize_rule_input(
    kind: &str,
    mode: &str,
    pattern: &str,
    action: &str,
) -> Result<(String, String, String, String), String> {
    let kind = require_one_of(kind, KINDS, "typ")?;
    let mode = require_one_of(mode, MODES, "režim")?;
    let action = require_one_of(action, ACTIONS, "akce")?;

    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err("Vzor nesmí být prázdný.".into());
    }

    let pattern = if kind == "site" {
        let (host, path) = rules::normalize_site_pattern(pattern).ok_or_else(|| {
            format!("„{pattern}\" není platná adresa. Zadejte doménu, např. reddit.com nebo reddit.com/r/rust.")
        })?;
        if path == "/" {
            host
        } else {
            format!("{host}{path}")
        }
    } else {
        pattern.to_string()
    };

    // `kill` is meaningless for a website and misleading in the UI.
    let action = if kind == "site" {
        "hide".to_string()
    } else {
        action
    };

    Ok((kind, mode, pattern, action))
}

// -----------------------------------------------------------------------------
// Session
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_focus_state(state: tauri::State<'_, AppState>) -> Result<FocusStateDto, String> {
    Ok(engine::state_dto(&state))
}

#[tauri::command]
pub async fn start_focus(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    duration_minutes: Option<i64>,
) -> Result<FocusStateDto, String> {
    engine::start(&app, &state, duration_minutes)
}

#[tauri::command]
pub async fn stop_focus(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<FocusStateDto, String> {
    engine::stop(&app, &state)
}

/// Start if stopped, stop if running. Backs the sidebar and popover buttons,
/// where a single control has to do both.
#[tauri::command]
pub async fn toggle_focus(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    duration_minutes: Option<i64>,
) -> Result<FocusStateDto, String> {
    // Held across the whole read-then-act sequence. `start` and `stop` are
    // synchronous, so nothing awaits while the guard is alive.
    let _serialised = state
        .focus_toggle_lock
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if engine::is_active(&state) {
        engine::stop(&app, &state)
    } else {
        engine::start(&app, &state, duration_minutes)
    }
}

// -----------------------------------------------------------------------------
// Settings
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_focus_settings(
    state: tauri::State<'_, AppState>,
) -> Result<FocusSettings, String> {
    Ok(engine::load_settings(&state.db))
}

#[tauri::command]
pub async fn set_focus_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: FocusSettings,
) -> Result<FocusSettings, String> {
    engine::save_settings(&state.db, &settings)?;
    // Strict-mode toggles change what the extension must block, so wake the
    // long-poll even when no session is running.
    engine::bump_generation(&app, &state);
    Ok(engine::load_settings(&state.db))
}

// -----------------------------------------------------------------------------
// Rules
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn list_focus_rules(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FocusRuleRow>, String> {
    cache::focus::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_focus_rule(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    kind: String,
    mode: String,
    pattern: String,
    label: Option<String>,
    action: Option<String>,
) -> Result<Vec<FocusRuleRow>, String> {
    let (kind, mode, pattern, action) =
        normalize_rule_input(&kind, &mode, &pattern, action.as_deref().unwrap_or("hide"))?;
    let label = label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());

    cache::focus::upsert(
        &state.db,
        cache::focus::NewFocusRule {
            kind: &kind,
            mode: &mode,
            pattern: &pattern,
            label: label.as_deref(),
            action: &action,
            enabled: true,
        },
        Utc::now().timestamp(),
    )
    .map_err(|e| e.to_string())?;

    engine::bump_generation(&app, &state);
    cache::focus::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_focus_rule_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<Vec<FocusRuleRow>, String> {
    cache::focus::set_enabled(&state.db, id, enabled).map_err(|e| e.to_string())?;
    engine::bump_generation(&app, &state);
    cache::focus::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_focus_rule_action(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    action: String,
) -> Result<Vec<FocusRuleRow>, String> {
    let action = require_one_of(&action, ACTIONS, "akce")?;
    // `action` only means anything for an application. `add_focus_rule`
    // already forces site rules to `hide`; without the same check here the two
    // entry points disagree and a site rule could carry a `kill` the engine
    // silently ignores.
    let rule = cache::focus::get(&state.db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Pravidlo neexistuje.".to_string())?;
    if rule.kind != "app" {
        return Err("Akci lze nastavit jen u pravidla pro aplikaci.".into());
    }
    cache::focus::set_action(&state.db, id, &action).map_err(|e| e.to_string())?;
    engine::bump_generation(&app, &state);
    cache::focus::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_focus_rule(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Vec<FocusRuleRow>, String> {
    cache::focus::delete(&state.db, id).map_err(|e| e.to_string())?;
    engine::bump_generation(&app, &state);
    cache::focus::list(&state.db).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Pickers / platform helpers
// -----------------------------------------------------------------------------

/// Apps the user could add a rule for. Safe-listed entries are returned too,
/// flagged as `protected`, so the UI can explain *why* Finder isn't blockable
/// instead of just hiding it.
#[tauri::command]
pub async fn list_running_apps() -> Result<Vec<RunningAppDto>, String> {
    let mut listed: Vec<RunningAppDto> = apps::running_apps()
        .into_iter()
        .filter_map(|app| {
            let pattern = app
                .bundle_id
                .clone()
                .or_else(|| app.exe.clone())
                .or_else(|| app.name.clone())?;
            Some(RunningAppDto {
                pattern,
                name: app.display_name(),
                pid: app.pid,
                protected: rules::is_safe_app(&app),
            })
        })
        .collect();
    listed.sort_by_key(|a| a.name.to_lowercase());
    Ok(listed)
}

/// The banner the overlay window should be showing.
///
/// The overlay webview is built on first use, so the event announcing the
/// first blocked app of a session is emitted before anything is listening.
/// Fetching on mount closes that gap.
#[tauri::command]
pub async fn get_focus_overlay_notice() -> Result<Option<OverlayNotice>, String> {
    Ok(crate::focus::overlay::last_notice())
}

/// macOS Shortcuts the user can bind to Focus start/stop. Empty elsewhere.
#[tauri::command]
pub async fn list_focus_shortcuts() -> Result<Vec<String>, String> {
    Ok(notify::list_macos_shortcuts())
}

/// Open the OS page where Do Not Disturb lives. The Windows fallback for
/// notification blocking.
#[tauri::command]
pub async fn open_dnd_settings() -> Result<(), String> {
    notify::open_system_dnd_settings()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_patterns_are_canonicalised() {
        let (kind, mode, pattern, action) =
            normalize_rule_input("site", "block", "  HTTPS://WWW.Reddit.com/  ", "kill").unwrap();
        assert_eq!(kind, "site");
        assert_eq!(mode, "block");
        assert_eq!(pattern, "reddit.com");
        // `kill` makes no sense for a website.
        assert_eq!(action, "hide");
    }

    #[test]
    fn site_pattern_keeps_a_path_prefix() {
        let (_, _, pattern, _) =
            normalize_rule_input("site", "block", "reddit.com/r/rust", "hide").unwrap();
        assert_eq!(pattern, "reddit.com/r/rust");
    }

    #[test]
    fn app_patterns_are_left_alone_apart_from_trimming() {
        let (_, _, pattern, action) =
            normalize_rule_input("app", "block", " com.slack.Slack ", "kill").unwrap();
        assert_eq!(pattern, "com.slack.Slack");
        assert_eq!(action, "kill");
    }

    #[test]
    fn unusable_site_pattern_is_rejected_with_an_explanation() {
        let err = normalize_rule_input("site", "block", "reddit", "hide").unwrap_err();
        assert!(err.contains("reddit.com"), "message should show an example");
    }

    #[test]
    fn empty_pattern_is_rejected() {
        assert!(normalize_rule_input("app", "block", "   ", "hide").is_err());
    }

    #[test]
    fn unknown_kind_mode_or_action_is_rejected() {
        assert!(normalize_rule_input("gadget", "block", "x.com", "hide").is_err());
        assert!(normalize_rule_input("site", "maybe", "x.com", "hide").is_err());
        assert!(normalize_rule_input("app", "block", "Slack", "nuke").is_err());
    }
}

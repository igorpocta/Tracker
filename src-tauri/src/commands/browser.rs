//! Browser extension bridge commands.
//!
//! In Phase 9 these are wired up to the local axum HTTP server
//! ([`crate::server`]). The extension itself doesn't exist yet, but the
//! plumbing on the desktop side is real:
//!
//! - The server bumps a heartbeat timestamp on every request — exposed via
//!   [`get_extension_last_heartbeat`].
//! - The server keeps the most recent "what ticket is the user looking at"
//!   payload — exposed via [`get_current_visible_ticket`].
//! - [`get_browser_context`] is a tiny convenience returning a URL string
//!   suitable for the UI's "open in Jira" affordance.

use serde::{Deserialize, Serialize};

use crate::server::{BrowserBridgeToken, ServerState};

/// Description of a Jira ticket currently visible in the user's browser.
/// Returned by [`get_current_visible_ticket`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibleTicket {
    pub issue_key: String,
    pub summary: Option<String>,
    pub url: Option<String>,
    /// Unix timestamp (seconds) when the extension last reported this ticket.
    pub seen_at: Option<i64>,
}

#[tauri::command]
pub async fn get_browser_context(
    state: tauri::State<'_, ServerState>,
) -> Result<Option<String>, String> {
    Ok(state
        .visible_ticket
        .read()
        .unwrap()
        .as_ref()
        .and_then(|t| t.url.clone()))
}

#[tauri::command]
pub async fn get_current_visible_ticket(
    state: tauri::State<'_, ServerState>,
) -> Result<Option<VisibleTicket>, String> {
    Ok(state
        .visible_ticket
        .read()
        .expect("WidgetState.visible_ticket RwLock poisoned")
        .clone())
}

#[tauri::command]
pub async fn get_extension_last_heartbeat(
    state: tauri::State<'_, ServerState>,
) -> Result<Option<i64>, String> {
    Ok(*state
        .last_heartbeat
        .read()
        .expect("WidgetState.last_heartbeat RwLock poisoned"))
}

/// Return the per-install bearer token the user must paste into their
/// browser extension's settings so the extension can reach the local
/// HTTP bridge. The token is generated once on first launch and persisted
/// in `appDataDir/browser-bridge-token` (Unix `0600`).
///
/// Exposed deliberately as an opt-in command so the FE settings panel
/// can show "Copy bridge token" — no other surface should leak this
/// value (no event broadcast, no logs).
#[tauri::command]
pub async fn get_browser_bridge_token(
    token: tauri::State<'_, BrowserBridgeToken>,
) -> Result<String, String> {
    Ok(token.as_str().to_string())
}

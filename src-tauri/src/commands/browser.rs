//! Browser extension bridge commands.
//!
//! For Phase 4 these are *stubs* returning empty values. The real
//! implementation lands in Phase 9 (local axum HTTP server + extension
//! handshake). We expose the surface now so the frontend can be wired up
//! against final command names and signatures.

use serde::{Deserialize, Serialize};

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
pub async fn get_browser_context() -> Result<Option<String>, String> {
    Ok(None)
}

#[tauri::command]
pub async fn get_current_visible_ticket() -> Result<Option<VisibleTicket>, String> {
    Ok(None)
}

#[tauri::command]
pub async fn get_extension_last_heartbeat() -> Result<Option<i64>, String> {
    Ok(None)
}

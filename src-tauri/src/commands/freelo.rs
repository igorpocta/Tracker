//! Freelo-specific Tauri commands (Phase 18E).
//!
//! - `list_freelo_projects(connection_id)` — fetch the live list of projects
//!   from the Freelo API for the picker UI (does NOT touch the cache).
//! - `set_freelo_selected_projects(connection_id, project_ids)` — persist the
//!   user's project selection inside `config_json`.
//! - `get_freelo_selected_projects(connection_id)` — read back the persisted
//!   list.
//! - `sync_freelo_now(connection_id)` — trigger an immediate sync of the
//!   selected projects + the last 30 days of work-reports.

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self};
use crate::commands::connections::FreeloConnectionConfig;
use crate::freelo::{models::FreeloProject, sync as freelo_sync};
use crate::state::{AppState, ProviderClient};

/// DTO returned to the UI; mirrors [`FreeloProject`] with a `selected` flag
/// pre-filled so the picker can show checked/unchecked state without a
/// second round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloProjectDto {
    pub id: i64,
    pub name: String,
    pub state: String,
    pub selected: bool,
}

fn freelo_config_for(
    state: &AppState,
    connection_id: i64,
) -> Result<(crate::freelo::FreeloClient, FreeloConnectionConfig), String> {
    let conns = state.connections.read().unwrap();
    let active = conns
        .iter()
        .find(|c| c.id == connection_id && c.enabled)
        .ok_or_else(|| "Freelo připojení není aktivní".to_string())?;
    match &active.client {
        ProviderClient::Freelo(client, cfg) => Ok((client.clone(), cfg.clone())),
        _ => Err("Připojení není Freelo".into()),
    }
}

/// Live list of projects the authenticated user can access. The `selected`
/// field is filled in from the persisted config so the UI doesn't have to
/// merge two responses.
#[tauri::command]
pub async fn list_freelo_projects(
    state: tauri::State<'_, AppState>,
    connection_id: i64,
) -> Result<Vec<FreeloProjectDto>, String> {
    let (client, cfg) = freelo_config_for(&state, connection_id)?;
    let projects: Vec<FreeloProject> = client.list_projects().await.map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(projects.len());
    for p in projects {
        let selected = cfg.selected_project_ids.contains(&p.id);
        out.push(FreeloProjectDto {
            id: p.id,
            name: p.name,
            state: p.state,
            selected,
        });
    }
    Ok(out)
}

/// Persist the user's project selection. Replaces the entire list (caller is
/// responsible for sending the full set, not deltas).
#[tauri::command]
pub async fn set_freelo_selected_projects(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: i64,
    project_ids: Vec<i64>,
) -> Result<(), String> {
    let row = cache::connections::get_by_id(&state.db, connection_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found".to_string())?;
    if row.provider != "freelo" {
        return Err("Připojení není Freelo".into());
    }
    let mut cfg: FreeloConnectionConfig =
        serde_json::from_str(&row.config_json).unwrap_or_default();
    // Dedupe and sort for deterministic JSON.
    let mut ids: Vec<i64> = project_ids;
    ids.sort_unstable();
    ids.dedup();
    cfg.selected_project_ids = ids;

    let cfg_json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    cache::connections::update_fields(&state.db, connection_id, None, None, Some(&cfg_json))
        .map_err(|e| e.to_string())?;
    let _ = state.hydrate_connections();
    let _ = app.emit("freelo-projects-changed", connection_id);
    Ok(())
}

/// Read the persisted project selection.
#[tauri::command]
pub async fn get_freelo_selected_projects(
    state: tauri::State<'_, AppState>,
    connection_id: i64,
) -> Result<Vec<i64>, String> {
    let row = cache::connections::get_by_id(&state.db, connection_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found".to_string())?;
    if row.provider != "freelo" {
        return Err("Připojení není Freelo".into());
    }
    let cfg: FreeloConnectionConfig =
        serde_json::from_str(&row.config_json).unwrap_or_default();
    Ok(cfg.selected_project_ids)
}

/// Force an immediate sync (issues + last 30 days of work-reports) for a
/// Freelo connection. Returns the count of upserted task rows.
#[tauri::command]
pub async fn sync_freelo_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: i64,
) -> Result<usize, String> {
    let (client, mut cfg) = freelo_config_for(&state, connection_id)?;
    let issues = freelo_sync::sync_issues_for_connection(
        &client,
        &state.db,
        &cfg.selected_project_ids,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Cache sync_user_id if missing.
    if cfg.sync_user_id.is_none() {
        if let Ok(u) = client.me().await {
            cfg.sync_user_id = Some(u.id);
            let cfg_json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
            let _ = cache::connections::update_fields(
                &state.db,
                connection_id,
                None,
                None,
                Some(&cfg_json),
            );
            let _ = state.hydrate_connections();
        }
    }

    if let Some(user_id) = cfg.sync_user_id {
        let today = chrono::Local::now().date_naive();
        let from = today - chrono::Duration::days(30);
        let _ = freelo_sync::sync_worklogs_for_range(
            &client,
            &state.db,
            user_id,
            from,
            today,
            &cfg.selected_project_ids,
        )
        .await;
    }

    let _ = app.emit("cache-refreshed", issues);
    Ok(issues)
}

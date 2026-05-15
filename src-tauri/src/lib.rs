pub mod cache;
pub mod commands;
pub mod config;
pub mod freelo;
pub mod jira;
pub mod keychain;
pub mod popover;
pub mod server;
pub mod state;
pub mod tray;
pub mod tray_pulse;
pub mod tray_ticker;
pub mod validation;

use chrono::{Duration as ChronoDuration, Local, NaiveTime, TimeZone};
use tauri::{Emitter, Manager};

use crate::state::{AppState, ProviderClient};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir is available");
            std::fs::create_dir_all(&app_data_dir).ok();

            let db_path = app_data_dir.join("tracker.db");
            let db = cache::Db::open(&db_path).expect("open db");
            let state = AppState::new(db, app_data_dir.clone());

            // Phase 18A: migrate legacy single-Jira config into the
            // connections table on first run. If a config.toml + token exist
            // but no rows in `connections`, create the first connection.
            //
            // Phase 18F: once the connections table has any rows, we STOP
            // reading the legacy `config.toml` entirely so the UI doesn't
            // render the same Jira account twice. The old file is renamed to
            // `config.toml.bak` for safety.
            let cfg_path = app_data_dir.join("config.toml");
            if cfg_path.exists() {
                let has_connections = cache::connections::list(&state.db)
                    .map(|rows| !rows.is_empty())
                    .unwrap_or(false);
                if has_connections {
                    // Migration already done — retire the legacy file so the
                    // duplicate card never resurfaces. Best-effort: ignore
                    // rename errors so this doesn't block startup.
                    let bak_path = app_data_dir.join("config.toml.bak");
                    let _ = std::fs::rename(&cfg_path, &bak_path);
                } else if let Ok(cfg) = config::load_from_path(&cfg_path) {
                    // First-time migration path.
                    // Legacy shims (so old commands still work).
                    *state.jira_config.write().unwrap() = Some(cfg.clone());
                    let _ = state.try_build_client();
                    let connection_cfg =
                        crate::commands::connections::JiraConnectionConfig {
                            base_url: cfg.base_url.clone(),
                            email: cfg.email.clone(),
                            sync_jql: None,
                            my_issues_jql: None,
                        };
                    let cfg_json = serde_json::to_string(&connection_cfg)
                        .unwrap_or_else(|_| "{}".into());
                    let new_id = cache::connections::insert(
                        &state.db,
                        cache::connections::NewConnection {
                            provider: "jira",
                            name: "Jira",
                            enabled: true,
                            config_json: &cfg_json,
                        },
                    )
                    .ok();
                    // Copy the legacy token into the new
                    // connection:N:token key so the multi-connection
                    // hydration finds it.
                    if let Some(id) = new_id {
                        if let Ok(Some(tok)) =
                            crate::keychain::load_jira_token(&app_data_dir)
                        {
                            let key = cache::connections::token_key(id);
                            let _ = crate::keychain::set(
                                &app_data_dir,
                                crate::keychain::KEYCHAIN_SERVICE,
                                &key,
                                &tok,
                            );
                        }
                    }
                    // Retire the legacy file now that migration is done.
                    let bak_path = app_data_dir.join("config.toml.bak");
                    let _ = std::fs::rename(&cfg_path, &bak_path);
                }
            }

            // Hydrate the in-memory connections list (no-op if no
            // connections are configured yet — the setup wizard will create
            // the first one).
            if let Err(e) = state.hydrate_connections() {
                tracing::warn!("hydrate_connections failed: {e}");
            }

            app.manage(state);

            let handle = app.handle().clone();
            if let Err(e) = crate::tray::setup(&handle) {
                tracing::warn!("tray setup failed: {e}");
            }
            if let Err(e) = crate::popover::setup(&handle) {
                tracing::warn!("popover setup failed: {e}");
            }
            crate::tray_ticker::spawn(handle.clone());

            crate::server::start(handle);

            // Phase 15: startup recovery for pending deletes.
            let recovery_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let state = recovery_handle.state::<AppState>();
                let cutoff = chrono::Utc::now().timestamp() - 30;
                let pending = match cache::worklogs::pending_deletes_older_than(&state.db, cutoff) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                for row in pending {
                    let (Some(local_id), Some(jira_id)) = (row.id, row.jira_worklog_id.clone())
                    else {
                        continue;
                    };
                    commands::worklog::commit_pending_delete(
                        &recovery_handle,
                        &state,
                        local_id,
                        &row.issue_key,
                        &jira_id,
                    )
                    .await;
                }
            });

            // Auto-sync issues + worklogs across all enabled connections.
            let auto_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let state = auto_handle.state::<AppState>();
                let active = state.connections.read().unwrap().clone();
                let total = active.len();
                // Phase 18B — Item 19: emit progress so the UI can show a
                // banner with "Synchronizuji s Jira…".
                let _ = auto_handle.emit(
                    "auto-sync-progress",
                    serde_json::json!({
                        "phase": "starting",
                        "current": 0,
                        "total": total,
                    }),
                );
                for (idx, active_conn) in active.into_iter().enumerate() {
                    let _ = auto_handle.emit(
                        "auto-sync-progress",
                        serde_json::json!({
                            "phase": "issues",
                            "current": idx,
                            "total": total,
                        }),
                    );
                    match active_conn.client {
                        ProviderClient::Jira(client) => {
                            let _ = jira::sync_issues_from_jira(&client, &state.db).await;

                            let me = match client.myself().await {
                                Ok(u) => u.account_id,
                                Err(_) => continue,
                            };
                            let today = chrono::Local::now().date_naive();
                            let from = today - chrono::Duration::days(30);
                            let _ = auto_handle.emit(
                                "auto-sync-progress",
                                serde_json::json!({
                                    "phase": "worklogs",
                                    "current": idx,
                                    "total": total,
                                }),
                            );
                            let _ = jira::worklog_sync::sync_worklogs_for_range(
                                &client, &state.db, &me, from, today,
                            )
                            .await;
                        }
                        ProviderClient::Freelo(client, cfg) => {
                            let _ = freelo::sync::sync_issues_for_connection(
                                &client,
                                &state.db,
                                &cfg.selected_project_ids,
                            )
                            .await;

                            let user_id = match cfg.sync_user_id {
                                Some(uid) => uid,
                                None => match client.me().await {
                                    Ok(u) => u.id,
                                    Err(_) => continue,
                                },
                            };
                            let today = chrono::Local::now().date_naive();
                            let from = today - chrono::Duration::days(30);
                            let _ = auto_handle.emit(
                                "auto-sync-progress",
                                serde_json::json!({
                                    "phase": "worklogs",
                                    "current": idx,
                                    "total": total,
                                }),
                            );
                            let _ = freelo::sync::sync_worklogs_for_range(
                                &client, &state.db, user_id, from, today,
                            )
                            .await;
                        }
                    }
                }

                let _ = auto_handle.emit("auto-sync-complete", ());
            });

            // Phase 18A — Item 9: emit a `day-rollover` event at local midnight.
            // The frontend listens and re-evaluates "today" boundaries.
            let rollover_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let now = Local::now();
                    let midnight = (now.date_naive() + ChronoDuration::days(1))
                        .and_time(NaiveTime::from_hms_opt(0, 0, 1).unwrap());
                    let next = Local
                        .from_local_datetime(&midnight)
                        .single()
                        .unwrap_or(now + ChronoDuration::days(1));
                    let wait_ms = (next - now).num_milliseconds().max(1000);
                    tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
                    let _ = rollover_handle.emit("day-rollover", ());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Legacy single-Jira config (kept for backwards compat).
            commands::config::has_config,
            commands::config::save_config,
            commands::config::enter_setup,
            commands::config::enter_main_app,
            commands::config::open_main_window,
            commands::config::test_jira_connection,
            commands::config::get_current_config,
            commands::config::update_config,
            commands::config::sign_out,
            // Multi-connection (Phase 18A).
            commands::connections::list_connections,
            commands::connections::add_connection,
            commands::connections::update_connection,
            commands::connections::remove_connection,
            commands::connections::enable_connection,
            commands::connections::test_connection_for_provider,
            commands::connections::list_my_issues,
            // Freelo (Phase 18E)
            commands::freelo::list_freelo_projects,
            commands::freelo::set_freelo_selected_projects,
            commands::freelo::get_freelo_selected_projects,
            commands::freelo::sync_freelo_now,
            // Timer
            commands::timer::get_timer_state,
            commands::timer::start_timer,
            commands::timer::stop_timer_inner,
            commands::timer::update_timer_start,
            commands::timer::update_timer_comment,
            commands::timer::assign_active_timer,
            // Favorites (Phase 18B — Item 26)
            commands::favorites::list_favorites,
            commands::favorites::add_favorite,
            commands::favorites::remove_favorite,
            commands::favorites::is_favorite,
            // Issues
            commands::issues::search_issues_cache,
            commands::issues::get_recent_issues,
            commands::issues::get_suggested_issues,
            commands::issues::refresh_cache,
            commands::issues::get_cache_stats,
            // Worklog
            commands::worklog::get_worklog_issues,
            commands::worklog::get_worklogs_for_range,
            commands::worklog::refresh_all,
            commands::worklog::create_manual_worklog,
            commands::worklog::update_worklog,
            commands::worklog::delete_worklog,
            commands::worklog::undo_delete_worklog,
            commands::worklog::move_worklog,
            commands::worklog::get_audit_log,
            commands::worklog::purge_audit_log,
            commands::worklog::restore_deleted_worklog,
            commands::worklog::revert_worklog_update,
            commands::worklog::retry_failed_audit_action,
            commands::worklog::assign_worklog_issue,
            commands::worklog::delete_local_only_worklog,
            // Misc
            commands::misc::open_jira_issue,
            commands::misc::open_url,
            // Prefs
            commands::prefs::get_daily_goal,
            commands::prefs::set_daily_goal,
            commands::prefs::set_widget_format,
            commands::prefs::set_app_icon,
            commands::prefs::get_hourly_rate,
            commands::prefs::set_hourly_rate,
            commands::prefs::get_theme,
            commands::prefs::set_theme,
            commands::prefs::get_font_size,
            commands::prefs::set_font_size,
            commands::prefs::get_density,
            commands::prefs::set_density,
            commands::prefs::get_accent_color,
            commands::prefs::set_accent_color,
            commands::prefs::get_currency,
            commands::prefs::set_currency,
            commands::prefs::get_palette_mode,
            commands::prefs::set_palette_mode,
            commands::prefs::get_day_timeline_visible,
            commands::prefs::set_day_timeline_visible,
            commands::prefs::get_earnings_visible,
            commands::prefs::set_earnings_visible,
            // Rounding (Phase 18A — Item 27)
            commands::rounding::get_rounding_mode,
            commands::rounding::set_rounding_mode,
            commands::rounding::get_rounding_interval_minutes,
            commands::rounding::set_rounding_interval_minutes,
            // Calendar (Phase 18A — Item 2)
            commands::calendar::get_working_week_mask,
            commands::calendar::set_working_week_mask,
            commands::calendar::list_non_working_days,
            commands::calendar::add_non_working_day,
            commands::calendar::remove_non_working_day,
            commands::calendar::is_working_day,
            // Activity (Phase 18A — Item 32)
            commands::activity::record_user_activity,
            commands::activity::get_daily_activity,
            commands::activity::get_activity_threshold_min,
            commands::activity::set_activity_threshold_min,
            // Browser extension
            commands::browser::get_browser_context,
            commands::browser::get_current_visible_ticket,
            commands::browser::get_extension_last_heartbeat,
            // Tray
            commands::tray::show_tray_popover,
            commands::tray::hide_tray_popover,
            commands::tray::toggle_tray_popover,
            commands::tray::set_tray_available,
            commands::misc::haptic_feedback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

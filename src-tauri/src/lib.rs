pub mod cache;
pub mod commands;
pub mod config;
pub mod jira;
pub mod keychain;
pub mod popover;
pub mod server;
pub mod state;
pub mod tray;
pub mod tray_ticker;

use tauri::{Emitter, Manager};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir is available");
            std::fs::create_dir_all(&app_data_dir).ok();

            let db_path = app_data_dir.join("tracker.db");
            let db = cache::Db::open(&db_path).expect("open db");
            let state = AppState::new(db);

            // Best-effort: load on-disk config and rebuild the Jira client.
            // Missing/invalid config is fine — the setup wizard will create it.
            let cfg_path = app_data_dir.join("config.toml");
            if cfg_path.exists() {
                if let Ok(cfg) = config::load_from_path(&cfg_path) {
                    *state.jira_config.write().unwrap() = Some(cfg);
                    let _ = state.try_build_client();
                }
            }

            app.manage(state);

            // Tray icon + background tooltip ticker. Both run unconditionally;
            // if the user has no Jira configured the tray simply sits idle.
            let handle = app.handle().clone();
            if let Err(e) = crate::tray::setup(&handle) {
                tracing::warn!("tray setup failed: {e}");
            }
            crate::tray_ticker::spawn(handle.clone());

            // Local HTTP server for the (future) browser extension.
            // Bind failures are logged inside; they never abort startup.
            crate::server::start(handle);

            // Auto-sync issues + worklogs shortly after startup. All errors
            // are swallowed: a fresh install with no Jira config simply
            // returns early, and transient network failures should never
            // prevent the user from interacting with the local UI.
            let auto_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Let the main window mount before hammering the network.
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let state = auto_handle.state::<AppState>();
                let Some(client) = state.jira_client_cloned() else {
                    return;
                };

                let _ = jira::sync_issues_from_jira(&client, &state.db).await;

                let me = match client.myself().await {
                    Ok(u) => u.account_id,
                    Err(_) => return,
                };
                let today = chrono::Local::now().date_naive();
                let from = today - chrono::Duration::days(30);
                let _ = jira::worklog_sync::sync_worklogs_for_range(
                    &client,
                    &state.db,
                    &me,
                    from,
                    today,
                )
                .await;

                let _ = auto_handle.emit("auto-sync-complete", ());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::has_config,
            commands::config::save_config,
            commands::config::enter_setup,
            commands::config::enter_main_app,
            commands::config::open_main_window,
            commands::config::test_jira_connection,
            commands::config::get_current_config,
            commands::config::update_config,
            commands::config::sign_out,
            commands::timer::get_timer_state,
            commands::timer::start_timer,
            commands::timer::stop_timer_inner,
            commands::timer::update_timer_start,
            commands::issues::search_issues_cache,
            commands::issues::get_recent_issues,
            commands::issues::get_suggested_issues,
            commands::issues::refresh_cache,
            commands::worklog::get_worklog_issues,
            commands::worklog::get_worklogs_for_range,
            commands::worklog::refresh_all,
            commands::misc::open_jira_issue,
            commands::misc::open_url,
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
            commands::browser::get_browser_context,
            commands::browser::get_current_visible_ticket,
            commands::browser::get_extension_last_heartbeat,
            commands::tray::show_tray_popover,
            commands::tray::hide_tray_popover,
            commands::tray::toggle_tray_popover,
            commands::tray::set_tray_available,
            commands::misc::haptic_feedback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

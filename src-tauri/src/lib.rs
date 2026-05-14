pub mod cache;
pub mod commands;
pub mod config;
pub mod jira;
pub mod keychain;
pub mod state;

use tauri::Manager;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::has_config,
            commands::config::save_config,
            commands::config::enter_setup,
            commands::config::enter_main_app,
            commands::config::open_main_window,
            commands::timer::get_timer_state,
            commands::timer::start_timer,
            commands::timer::stop_timer_inner,
            commands::timer::update_timer_start,
            commands::issues::search_issues_cache,
            commands::issues::get_recent_issues,
            commands::issues::get_suggested_issues,
            commands::issues::refresh_cache,
            commands::worklog::get_worklog_issues,
            commands::misc::open_jira_issue,
            commands::misc::open_url,
            commands::prefs::get_daily_goal,
            commands::prefs::set_daily_goal,
            commands::prefs::set_widget_format,
            commands::prefs::set_app_icon,
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

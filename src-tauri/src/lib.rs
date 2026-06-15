pub mod app_icon;
pub mod audit_helpers;
pub mod cache;
pub mod commands;
pub mod config;
pub mod freelo;
pub mod http_base;
pub mod jira;
pub mod keychain;
pub mod popover;
pub mod sentry_init;
pub mod server;
pub mod state;
pub mod time;
pub mod tray;
pub mod tray_pulse;
pub mod tray_ticker;
pub mod validation;
pub mod worklog_service;

use chrono::{Duration as ChronoDuration, Local, NaiveTime, TimeZone};
use tauri::{Emitter, Manager};

use crate::state::AppState;

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
                    *state.jira_config.write().expect("AppState.jira_config RwLock poisoned") = Some(cfg.clone());
                    let _ = state.try_build_client();
                    let connection_cfg =
                        crate::commands::connections::JiraConnectionConfig {
                            base_url: cfg.base_url.clone(),
                            email: cfg.email.clone(),
                            ..Default::default()
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

            // Phase 19 — opt-in Sentry. We init BEFORE managing the rest of
            // the setup hook so any later panic / `tracing::error!` is
            // captured. The init is a no-op when:
            //   * the user has not flipped `sentry_enabled` to `true`, OR
            //   * no DSN is configured (build-time or runtime env var).
            //
            // The install id is generated lazily here so first-launch
            // reports always carry a stable identifier.
            {
                let enabled =
                    crate::commands::sentry::get_sentry_enabled_inner(&state.db)
                        .unwrap_or(false);
                let install_id =
                    crate::commands::sentry::get_or_create_install_id_inner(&state.db)
                        .unwrap_or_else(|_| String::new());
                if !install_id.is_empty() {
                    let _ = crate::sentry_init::init_if_enabled(&install_id, enabled);
                }
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
                    let Some(local_id) = row.id else { continue };
                    let Some(remote_id) = row.remote_id.clone() else { continue };
                    let issue_key = row.issue_key.clone().unwrap_or_default();
                    commands::worklog::commit_pending_delete(
                        &recovery_handle,
                        &state,
                        local_id,
                        &issue_key,
                        &remote_id,
                    )
                    .await;
                }
            });

            // Startup flush for unsynced-but-assigned worklogs (rows whose
            // original POST didn't land — HTTP bridge crash, sub-minute
            // Freelo rejection, app quit before the push completed, etc.).
            // Shared with refresh_all and the periodic auto-sync so the
            // flush logic lives in one place.
            let flush_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let state = flush_handle.state::<AppState>();
                let _ = commands::worklog::flush_unsynced_worklogs(&flush_handle, &state).await;
            });

            // Auto-sync issues + worklogs across all enabled connections.
            //
            // Phase 22 — proper periodic loop (was a one-shot at startup). The
            // interval is user-configurable via `set_auto_sync_interval_seconds`
            // (Settings → Obecné → Reindex). Setting `0` disables the periodic
            // sync entirely; the loop then idles, re-checking the pref every
            // five minutes so a flip back to a non-zero interval kicks in
            // without an app restart.
            //
            // Sliding interval semantics: each iteration sleeps `interval_secs`
            // AFTER the previous sync finishes, so a 1h interval means roughly
            // "1h between syncs", not "fire on every wall-clock hour".
            let auto_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let state = auto_handle.state::<AppState>();

                // Dev-time throttle for the FIRST iteration only: skip if the
                // last sync happened within N minutes. Prevents a `tauri dev`
                // rebuild loop from hammering the API. Subsequent iterations
                // are gated by the proper sleep interval instead.
                //
                //   - Default 60 minutes in debug builds.
                //   - Disabled in release builds.
                //   - Override via `TRACKER_SYNC_THROTTLE_MIN=<n>`
                //     (`0` forces every restart to sync).
                let throttle_min: i64 = std::env::var("TRACKER_SYNC_THROTTLE_MIN")
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(if cfg!(debug_assertions) { 60 } else { 0 });
                let throttle_secs = throttle_min.max(0) * 60;
                let mut first_iteration = true;

                loop {
                    // Pref is re-read each iteration so changes from the UI
                    // take effect on the next cycle (no restart needed).
                    let interval_secs = commands::prefs::get_auto_sync_interval_inner(&state.db)
                        .map(|s| s.max(0) as u64)
                        .unwrap_or(
                            commands::prefs::DEFAULT_AUTO_SYNC_INTERVAL_SECONDS as u64,
                        );

                    if interval_secs == 0 {
                        // Manual mode — skip the sync, recheck pref in 5 min so
                        // toggling back on isn't gated on a long sleep.
                        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                        first_iteration = false;
                        continue;
                    }

                    // Drain unsynced backlog before pulling. Bounded
                    // per iteration so a long-offline session catches
                    // up gradually rather than fanning out to hundreds
                    // of parallel POSTs.
                    let _ = commands::worklog::flush_unsynced_worklogs(&auto_handle, &state).await;

                    // Snapshot active connections per-iteration so new/removed
                    // connections are picked up without an app restart.
                    let active = state
                        .connections
                        .read()
                        .expect("AppState.connections RwLock poisoned")
                        .clone();
                    let total = active.len();
                    let now_unix = chrono::Utc::now().timestamp();

                    for (idx, active_conn) in active.into_iter().enumerate() {
                        let throttle_key = format!("last_auto_sync_at:{}", active_conn.id);
                        if first_iteration && throttle_secs > 0 {
                            let last = cache::settings::get(&state.db, &throttle_key)
                                .ok()
                                .flatten()
                                .and_then(|s| s.parse::<i64>().ok())
                                .unwrap_or(0);
                            if now_unix - last < throttle_secs {
                                tracing::info!(
                                    target: "auto_sync",
                                    "throttle: skipping connection id={} (last sync {}s ago, threshold {}s)",
                                    active_conn.id,
                                    now_unix - last,
                                    throttle_secs,
                                );
                                continue;
                            }
                        }

                        let _ = commands::worklog::sync_one_connection(
                            &auto_handle,
                            &state.db,
                            active_conn,
                            idx,
                            total,
                            commands::worklog::SyncMode::Incremental,
                        )
                        .await;

                        let _ = cache::settings::set(
                            &state.db,
                            &throttle_key,
                            &now_unix.to_string(),
                        );
                    }

                    let _ = auto_handle.emit("auto-sync-complete", ());
                    first_iteration = false;
                    tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
                }
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

            // Autostart launches the app with `--minimized` (see the
            // autostart plugin registration above). The main window is
            // configured `visible: true`, so without this it would pop up on
            // login. Hide it here; the tray/dock keeps the app reachable.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

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
            commands::connections::list_jira_statuses,
            commands::connections::get_connection_stats,
            // Freelo (Phase 18E)
            commands::freelo::list_freelo_projects,
            commands::freelo::set_freelo_selected_projects,
            commands::freelo::get_freelo_selected_projects,
            commands::freelo::sync_freelo_now,
            // Timer
            commands::timer::get_timer_state,
            commands::timer::start_timer,
            commands::timer::stop_timer_inner,
            commands::timer::discard_timer,
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
            commands::worklog::refresh_connection,
            commands::worklog::get_sync_errors,
            commands::worklog::list_sync_runs,
            commands::worklog::split_worklog,
            commands::dashboard::get_jira_dashboard_issues,
            commands::streaks::get_streaks,
            commands::suggestions::get_suggestions,
            commands::backup::export_backup,
            commands::backup::import_backup,
            commands::worklog::create_manual_worklog,
            commands::worklog::update_worklog,
            commands::worklog::update_local_worklog,
            commands::worklog::push_local_worklog,
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
            commands::misc::open_issue,
            commands::misc::open_url,
            // Prefs
            commands::prefs::get_daily_goal,
            commands::prefs::set_daily_goal,
            commands::prefs::get_auto_sync_interval_seconds,
            commands::prefs::set_auto_sync_interval_seconds,
            commands::prefs::get_smart_suggestions_enabled,
            commands::prefs::set_smart_suggestions_enabled,
            commands::prefs::get_pomodoro,
            commands::prefs::set_pomodoro,
            commands::prefs::list_project_colors,
            commands::prefs::set_project_color,
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
            commands::system_idle::get_system_idle_seconds,
            // Browser extension
            commands::browser::get_browser_context,
            commands::browser::get_current_visible_ticket,
            commands::browser::get_extension_last_heartbeat,
            commands::browser::get_browser_bridge_token,
            // Tray
            commands::tray::show_tray_popover,
            commands::tray::hide_tray_popover,
            commands::tray::toggle_tray_popover,
            commands::tray::set_tray_available,
            commands::tray::quit_app,
            commands::tray::set_app_icon_accent,
            commands::misc::haptic_feedback,
            // Phase 19 — Sentry opt-in
            commands::sentry::get_sentry_enabled,
            commands::sentry::set_sentry_enabled,
            commands::sentry::get_install_id,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // macOS-specifický fix: klik na dock ikonu (případně otevření
            // appky když je už spuštěná) emituje `RunEvent::Reopen`. Pokud
            // všechna naše okna jsou skryta (typicky uživatel zavřel main
            // okno červeným tlačítkem), výchozí chování by appku jen
            // přepnulo dopředu beze změny viditelnosti — uživatel uvidí
            // ikonu v doku, ale žádné okno. Tento handler v takovém případě
            // vynutí zobrazení main okna, aby se appka "probudila".
            // `RunEvent::Reopen` existuje jen na macOS, na ostatních
            // platformách tahle varianta není a build by spadl.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = _event
            {
                if !has_visible_windows {
                    let _ = crate::popover::open_main(_app_handle);
                }
            }
        });
}

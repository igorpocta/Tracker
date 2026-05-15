//! Integration test for the post-import state refresh.
//!
//! Pre-fix, `import_backup` updated the DB but left the runtime
//! `AppState` (connections list + legacy single-Jira shims) frozen
//! on the pre-restore state. The app kept making mutations against
//! whichever client was loaded at launch — so a freshly-restored
//! tenant was invisible to the live runtime until the user
//! restarted, and a tenant that was DROPPED in the restored bundle
//! still had a live HTTP client in memory.
//!
//! The fix extracts `apply_post_import_state_refresh` from the
//! Tauri command so this test can exercise it without spinning up
//! a Tauri runtime. The event emits (`connections-changed`,
//! `config-changed`, `cache-refreshed`) live in the command and
//! are covered by a manual-test smoke check; the structural
//! "shim-reset + re-hydrate" half is what this file pins.

use tempfile::TempDir;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::Db;
use tracker_lib::commands::backup::apply_post_import_state_refresh;
use tracker_lib::config::JiraConfig;
use tracker_lib::state::{ActiveConnection, AppState, ProviderClient};

#[test]
fn state_refresh_clears_legacy_shims_and_rehydrates_from_db() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let state = AppState::new(db, dir.path().to_path_buf());

    // Pre-import: app is running with the OLD connection (`pre-restore`).
    // We stamp both the new-API list (state.connections) and the legacy
    // single-Jira shims so the test mirrors what a real running app
    // looks like after a successful first launch.
    let pre_id = insert_conn(
        &state.db,
        NewConnection {
            provider: "jira",
            name: "pre-restore",
            enabled: true,
            config_json: r#"{"base_url":"https://pre.example/","email":"old@example.com"}"#,
        },
    )
    .unwrap();

    let pre_client = tracker_lib::jira::JiraClient::new(
        "https://pre.example".into(),
        "old@example.com".into(),
        "pre-token".into(),
    )
    .unwrap();
    {
        let mut conns = state.connections.write().unwrap();
        conns.push(ActiveConnection {
            id: pre_id,
            kind: "jira".into(),
            name: "pre-restore".into(),
            enabled: true,
            client: ProviderClient::Jira(pre_client.clone()),
        });
    }
    *state.jira_config.write().unwrap() = Some(JiraConfig {
        base_url: "https://pre.example".into(),
        email: "old@example.com".into(),
    });
    *state.jira_client.write().unwrap() = Some(pre_client);

    // Sanity: the live state matches the pre-import setup.
    assert_eq!(state.connections.read().unwrap().len(), 1);
    assert!(state.jira_config.read().unwrap().is_some());
    assert!(state.jira_client.read().unwrap().is_some());

    // Now SIMULATE an `import_inner` outcome: we replace the connections
    // table contents with a different bundle. The point is to give
    // `apply_post_import_state_refresh` a fresh source-of-truth to read
    // from.
    state
        .db
        .pool()
        .get()
        .unwrap()
        .execute("DELETE FROM connections", [])
        .unwrap();
    let post_id = insert_conn(
        &state.db,
        NewConnection {
            provider: "jira",
            name: "post-restore",
            enabled: true,
            config_json: r#"{"base_url":"https://post.example/","email":"new@example.com"}"#,
        },
    )
    .unwrap();
    assert_ne!(pre_id, post_id, "ids should differ");

    // Run the refresh.
    apply_post_import_state_refresh(&state);

    // Legacy shims are cleared. `hydrate_connections` only re-populates
    // them when it finds an enabled Jira connection AND has a token in
    // the keychain. The test environment has no keychain entry under
    // `connection:<post_id>:token`, so we expect the shims to stay
    // empty here. The important guarantee is that the OLD shims (pre-
    // restore) are gone — i.e. the cross-tenant write hazard is gone.
    assert!(
        state.jira_client.read().unwrap().is_none(),
        "post-import jira_client shim must not retain the pre-restore client",
    );
    assert!(
        state.jira_config.read().unwrap().is_none(),
        "post-import jira_config shim must not retain the pre-restore config",
    );

    // The live `connections` list is rebuilt from the DB. Tokens are
    // missing for the new id (see above), so hydrate skips them, but
    // the pre-restore entry MUST be gone — otherwise mutations would
    // still target the old tenant.
    let live = state.connections.read().unwrap();
    assert!(
        live.iter().all(|c| c.id != pre_id),
        "old connection must not survive the import refresh: {:?}",
        live.iter().map(|c| c.id).collect::<Vec<_>>()
    );
}

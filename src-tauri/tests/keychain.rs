//! Secret-store integration tests.
//!
//! Since Phase 17 the "keychain" is just a TOML file in the app data dir, so
//! these tests are fully hermetic — they use a `TempDir` and run without any
//! OS prompts.

use tempfile::TempDir;

#[test]
fn low_level_set_get_delete_roundtrip() {
    let dir = TempDir::new().unwrap();
    let svc = "com.tracker.app.test.low";
    let key = "test-token-low";

    tracker_lib::keychain::set(dir.path(), svc, key, "secret-xyz").unwrap();
    let got = tracker_lib::keychain::get(dir.path(), svc, key).unwrap();
    assert_eq!(got.as_deref(), Some("secret-xyz"));

    tracker_lib::keychain::delete(dir.path(), svc, key).unwrap();
    assert!(tracker_lib::keychain::get(dir.path(), svc, key)
        .unwrap()
        .is_none());
}

#[test]
fn jira_token_helpers_roundtrip() {
    let dir = TempDir::new().unwrap();

    assert!(tracker_lib::keychain::load_jira_token(dir.path())
        .unwrap()
        .is_none());

    tracker_lib::keychain::save_jira_token(dir.path(), "abc-123").unwrap();
    let got = tracker_lib::keychain::load_jira_token(dir.path()).unwrap();
    assert_eq!(got.as_deref(), Some("abc-123"));

    tracker_lib::keychain::clear_jira_token(dir.path()).unwrap();
    assert!(tracker_lib::keychain::load_jira_token(dir.path())
        .unwrap()
        .is_none());
}

#[test]
fn multiple_services_coexist_in_one_file() {
    let dir = TempDir::new().unwrap();
    tracker_lib::keychain::set(dir.path(), "svc.a", "alpha", "AAA").unwrap();
    tracker_lib::keychain::set(dir.path(), "svc.b", "beta", "BBB").unwrap();

    assert_eq!(
        tracker_lib::keychain::get(dir.path(), "svc.a", "alpha")
            .unwrap()
            .as_deref(),
        Some("AAA")
    );
    assert_eq!(
        tracker_lib::keychain::get(dir.path(), "svc.b", "beta")
            .unwrap()
            .as_deref(),
        Some("BBB")
    );

    // Removing one leaves the other intact.
    tracker_lib::keychain::delete(dir.path(), "svc.a", "alpha").unwrap();
    assert!(tracker_lib::keychain::get(dir.path(), "svc.a", "alpha")
        .unwrap()
        .is_none());
    assert_eq!(
        tracker_lib::keychain::get(dir.path(), "svc.b", "beta")
            .unwrap()
            .as_deref(),
        Some("BBB")
    );
}

#[test]
fn delete_on_missing_entry_is_idempotent() {
    let dir = TempDir::new().unwrap();
    // Nothing has ever been written; delete must not error.
    tracker_lib::keychain::clear_jira_token(dir.path()).unwrap();
    tracker_lib::keychain::delete(dir.path(), "no.such", "absent").unwrap();
}

#[test]
fn secret_file_lives_inside_app_data_dir() {
    let dir = TempDir::new().unwrap();
    tracker_lib::keychain::save_jira_token(dir.path(), "tok").unwrap();
    let p = dir.path().join("secret.toml");
    assert!(p.exists(), "secret.toml should be created in app_data_dir");
}

#[cfg(unix)]
#[test]
fn secret_file_has_0600_permissions_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    tracker_lib::keychain::save_jira_token(dir.path(), "secret").unwrap();

    let meta = std::fs::metadata(dir.path().join("secret.toml")).unwrap();
    // Lower 9 bits encode the rwx triplet. Expect rw------- (0o600).
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "secret.toml must be chmod 0600, got 0o{mode:o}"
    );
}

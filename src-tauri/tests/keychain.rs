//! Keychain integration tests.
//!
//! These tests touch the real OS keychain and on macOS will prompt the user
//! for access on first run, so they are marked `#[ignore]` and only run with
//! `cargo test -- --include-ignored`.

#[test]
#[ignore = "requires OS keychain access; run with --include-ignored locally"]
fn low_level_set_get_delete_roundtrip() {
    let svc = "com.tracker.app.test.low";
    let key = "test-token-low";
    tracker_lib::keychain::set(svc, key, "secret-xyz").unwrap();
    let got = tracker_lib::keychain::get(svc, key).unwrap();
    assert_eq!(got.as_deref(), Some("secret-xyz"));
    tracker_lib::keychain::delete(svc, key).unwrap();
    assert!(tracker_lib::keychain::get(svc, key).unwrap().is_none());
}

#[test]
#[ignore = "requires OS keychain access; run with --include-ignored locally"]
fn jira_token_helpers_roundtrip() {
    // Use a unique test service constant to avoid collision with user's real token.
    // Since the helpers use the production constants, ensure we clear first/last.
    tracker_lib::keychain::clear_jira_token().unwrap();
    tracker_lib::keychain::save_jira_token("abc-123").unwrap();
    let got = tracker_lib::keychain::load_jira_token().unwrap();
    assert_eq!(got.as_deref(), Some("abc-123"));
    tracker_lib::keychain::clear_jira_token().unwrap();
    assert!(tracker_lib::keychain::load_jira_token().unwrap().is_none());
}

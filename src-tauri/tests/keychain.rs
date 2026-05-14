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

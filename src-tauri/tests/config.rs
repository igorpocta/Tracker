//! Tests for the on-disk `JiraConfig` (TOML serde + path I/O).

use tempfile::TempDir;
use tracker_lib::config::{load_from_path, save_to_path, JiraConfig};

#[test]
fn jira_config_toml_roundtrip() {
    let cfg = JiraConfig {
        base_url: "https://example.atlassian.net".into(),
        email: "user@example.com".into(),
    };
    let toml_text = toml::to_string(&cfg).unwrap();
    let parsed: JiraConfig = toml::from_str(&toml_text).unwrap();
    assert_eq!(parsed.base_url, cfg.base_url);
    assert_eq!(parsed.email, cfg.email);
}

#[test]
fn save_then_load_via_path_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested").join("config.toml");

    let cfg = JiraConfig {
        base_url: "https://acme.atlassian.net".into(),
        email: "alice@acme.example".into(),
    };

    save_to_path(&path, &cfg).expect("save");
    assert!(path.exists());

    let loaded = load_from_path(&path).expect("load");
    assert_eq!(loaded.base_url, cfg.base_url);
    assert_eq!(loaded.email, cfg.email);
}

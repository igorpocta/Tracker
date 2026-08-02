//! Pure rule evaluation for Focus mode.
//!
//! Nothing here touches the OS or the database — the engine hands in a
//! snapshot of the enabled rules plus whatever it observed, and gets back a
//! decision. That keeps the interesting logic (precedence, subdomain matching,
//! the safe-list) unit-testable without a running desktop.
//!
//! ## Precedence
//!
//! `allow` always beats `block`, so a narrow exception can be carved out of a
//! broad blocking rule (`block reddit.com` + `allow reddit.com/r/rust`).
//! The strict (allow-list) flag only decides what happens when *nothing*
//! matched.

use crate::cache::focus::FocusRuleRow;

/// Bundle identifiers that enforcement must never touch on macOS. Killing or
/// hiding any of these degrades the desktop itself rather than the user's
/// concentration.
pub const MACOS_SAFE_LIST: &[&str] = &[
    "com.apple.finder",
    "com.apple.dock",
    "com.apple.systemuiserver",
    "com.apple.controlcenter",
    "com.apple.notificationcenterui",
    "com.apple.loginwindow",
    "com.apple.windowmanager",
    "com.apple.spotlight",
    "com.apple.systempreferences",
    "com.apple.systemsettings",
    "com.tracker.app",
];

/// Executable names that enforcement must never touch on Windows. Same
/// reasoning as [`MACOS_SAFE_LIST`]; `explorer.exe` in particular *is* the
/// desktop shell.
pub const WINDOWS_SAFE_LIST: &[&str] = &[
    "explorer.exe",
    "dwm.exe",
    "winlogon.exe",
    "csrss.exe",
    "lsass.exe",
    "services.exe",
    "smss.exe",
    "wininit.exe",
    "sihost.exe",
    "ctfmon.exe",
    "taskmgr.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "tracker.exe",
];

/// What the engine should do with a blocked app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    /// Push the window out of the way. Never loses unsaved work.
    Hide,
    /// Terminate the process. Only ever chosen by an explicit `block` rule.
    Kill,
}

impl AppAction {
    pub fn parse(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case("kill") {
            AppAction::Kill
        } else {
            AppAction::Hide
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDecision {
    Allowed,
    Blocked(AppAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteDecision {
    Allowed,
    Blocked,
}

/// Everything we know about a foreground application. All three identifiers
/// are matched against a rule's pattern, so a single rule works across
/// platforms without the user maintaining two entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppIdent {
    /// macOS bundle identifier, e.g. `com.slack.Slack`.
    pub bundle_id: Option<String>,
    /// Windows executable file name, e.g. `slack.exe`.
    pub exe: Option<String>,
    /// Human-facing name, e.g. `Slack`.
    pub name: Option<String>,
    /// OS process id. `0` when unknown.
    pub pid: i64,
}

impl AppIdent {
    /// Best label for UI / logs.
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.exe.clone())
            .or_else(|| self.bundle_id.clone())
            .unwrap_or_else(|| format!("PID {}", self.pid))
    }
}

/// `true` when this app is on the platform safe-list and must never be
/// hidden or killed. Both lists are checked regardless of the host platform —
/// the identifier namespaces don't overlap, and it keeps the function pure.
pub fn is_safe_app(app: &AppIdent) -> bool {
    let bundle = app.bundle_id.as_deref().unwrap_or("").to_ascii_lowercase();
    if !bundle.is_empty() && MACOS_SAFE_LIST.contains(&bundle.as_str()) {
        return true;
    }
    let exe = app.exe.as_deref().unwrap_or("").to_ascii_lowercase();
    if !exe.is_empty() && WINDOWS_SAFE_LIST.contains(&exe.as_str()) {
        return true;
    }
    false
}

/// Does `pattern` name this app? Compared case-insensitively against the
/// bundle id, the executable name (with and without the `.exe` suffix) and
/// the display name.
pub fn app_matches(app: &AppIdent, pattern: &str) -> bool {
    let needle = pattern.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return false;
    }
    let needle_bare = needle.strip_suffix(".exe").unwrap_or(&needle);

    let candidates = [
        app.bundle_id.as_deref(),
        app.exe.as_deref(),
        app.name.as_deref(),
    ];
    for candidate in candidates.into_iter().flatten() {
        let hay = candidate.trim().to_ascii_lowercase();
        if hay.is_empty() {
            continue;
        }
        if hay == needle {
            return true;
        }
        if hay.strip_suffix(".exe").unwrap_or(&hay) == needle_bare {
            return true;
        }
    }
    false
}

/// Is there at least one enabled `allow` rule of this kind?
///
/// Strict mode hinges on this. An empty allow-list does not mean "allow
/// nothing" — it means the user flipped the switch before filling the list in,
/// and honouring it literally would hide every window on the machine one per
/// second (or block every site) with no hint as to why. Treating it as "not
/// configured yet" is the only reading that isn't a trap.
fn has_enabled_allow(rules: &[FocusRuleRow], kind: &str) -> bool {
    rules
        .iter()
        .any(|r| r.kind == kind && r.mode == "allow" && r.enabled)
}

/// Decide what to do with the app the user just brought to the foreground.
///
/// Strict mode never escalates to [`AppAction::Kill`] — an allow-list is a
/// broad net and terminating everything the user forgot to whitelist would be
/// hostile. Explicit `block` rules are the only path to a kill.
pub fn decide_app(app: &AppIdent, rules: &[FocusRuleRow], strict: bool) -> AppDecision {
    if is_safe_app(app) {
        return AppDecision::Allowed;
    }
    let strict = strict && has_enabled_allow(rules, "app");

    let app_rules = rules.iter().filter(|r| r.kind == "app" && r.enabled);

    let mut blocked_action: Option<AppAction> = None;
    for rule in app_rules {
        if !app_matches(app, &rule.pattern) {
            continue;
        }
        if rule.mode == "allow" {
            return AppDecision::Allowed;
        }
        if rule.mode == "block" && blocked_action.is_none() {
            blocked_action = Some(AppAction::parse(&rule.action));
        }
    }

    match blocked_action {
        Some(action) => AppDecision::Blocked(action),
        None if strict => AppDecision::Blocked(AppAction::Hide),
        None => AppDecision::Allowed,
    }
}

/// Split a URL into `(host, path)` with both lowercased. Returns `None` for
/// anything that doesn't look like an absolute http(s) URL — the engine skips
/// those rather than guessing.
///
/// Hand-rolled instead of using the `url` crate so the same helper can parse
/// user-typed rule patterns, which are not valid URLs.
pub fn split_url(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    let without_scheme = match trimmed.find("://") {
        Some(idx) => {
            let scheme = trimmed[..idx].to_ascii_lowercase();
            if scheme != "http" && scheme != "https" {
                return None;
            }
            &trimmed[idx + 3..]
        }
        // Rule patterns arrive bare (`reddit.com/r/rust`).
        None => trimmed,
    };
    if without_scheme.is_empty() {
        return None;
    }

    let authority_end = without_scheme
        .find(['/', '?', '#'])
        .unwrap_or(without_scheme.len());
    let authority = &without_scheme[..authority_end];
    // Drop `user:pass@` — it is not part of the host and would defeat matching.
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    // IPv6 literals keep their brackets; only strip a port from the tail.
    let host_raw = if host_port.starts_with('[') {
        match host_port.find(']') {
            Some(end) => &host_port[..=end],
            None => host_port,
        }
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    let host = host_raw.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    let rest = &without_scheme[authority_end..];
    let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = rest[..path_end].to_ascii_lowercase();
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path
    };

    Some((host, path))
}

/// `true` for loopback hosts, which are never blocked — the block page itself
/// lives on `127.0.0.1` and dead-ending it would strand the browser.
pub fn is_loopback_host(host: &str) -> bool {
    matches!(
        host,
        "localhost" | "127.0.0.1" | "0.0.0.0" | "[::1]" | "::1"
    ) || host.ends_with(".localhost")
        || host.starts_with("127.")
}

/// Normalise a user-typed site pattern into `(host, path_prefix)`.
///
/// Accepts `example.com`, `*.example.com`, `www.example.com`,
/// `https://example.com/path` and everything in between; the leading `*.`
/// and `www.` are dropped because subdomain matching is implicit anyway.
pub fn normalize_site_pattern(raw: &str) -> Option<(String, String)> {
    let (parsed_host, path) = split_url(raw)?;
    let mut host = parsed_host.as_str();
    if let Some(rest) = host.strip_prefix("*.") {
        host = rest;
    }
    if let Some(rest) = host.strip_prefix("www.") {
        host = rest;
    }
    let host = host.to_string();
    if host.is_empty() || (!host.contains('.') && host != "localhost") {
        // Reject bare words like "reddit" — they would match nothing useful
        // and silently look broken.
        return None;
    }
    Some((host, path))
}

/// Does `(host, path)` fall under the rule pattern? Subdomains match
/// implicitly, and a pattern path acts as a prefix.
pub fn site_matches(host: &str, path: &str, pattern: &str) -> bool {
    let Some((p_host, p_path)) = normalize_site_pattern(pattern) else {
        return false;
    };
    let host_ok = host == p_host || host.ends_with(&format!(".{p_host}"));
    if !host_ok {
        return false;
    }
    if p_path == "/" {
        return true;
    }
    path.starts_with(&p_path)
}

/// Decide whether a URL may be visited during a focus session.
pub fn decide_site(url: &str, rules: &[FocusRuleRow], strict: bool) -> SiteDecision {
    let Some((host, path)) = split_url(url) else {
        // Not an http(s) URL (about:blank, chrome://…, a file) — leave it be.
        return SiteDecision::Allowed;
    };
    if is_loopback_host(&host) {
        return SiteDecision::Allowed;
    }
    let strict = strict && has_enabled_allow(rules, "site");

    let mut blocked = false;
    for rule in rules.iter().filter(|r| r.kind == "site" && r.enabled) {
        if !site_matches(&host, &path, &rule.pattern) {
            continue;
        }
        if rule.mode == "allow" {
            return SiteDecision::Allowed;
        }
        if rule.mode == "block" {
            blocked = true;
        }
    }

    if blocked || strict {
        SiteDecision::Blocked
    } else {
        SiteDecision::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: &str, mode: &str, pattern: &str, action: &str) -> FocusRuleRow {
        FocusRuleRow {
            id: 0,
            kind: kind.into(),
            mode: mode.into(),
            pattern: pattern.into(),
            label: None,
            action: action.into(),
            enabled: true,
            created_at: 0,
        }
    }

    fn app(bundle: &str, exe: &str, name: &str) -> AppIdent {
        AppIdent {
            bundle_id: Some(bundle.into()),
            exe: Some(exe.into()),
            name: Some(name.into()),
            pid: 42,
        }
    }

    // ----- URL parsing -------------------------------------------------------

    #[test]
    fn split_url_extracts_lowercase_host_and_path() {
        assert_eq!(
            split_url("https://Sub.Example.COM:8443/R/Rust?x=1#frag"),
            Some(("sub.example.com".into(), "/r/rust".into()))
        );
    }

    #[test]
    fn split_url_defaults_missing_path_to_root() {
        assert_eq!(
            split_url("https://example.com"),
            Some(("example.com".into(), "/".into()))
        );
    }

    #[test]
    fn split_url_drops_userinfo() {
        assert_eq!(
            split_url("https://user:pass@evil.example.com/x"),
            Some(("evil.example.com".into(), "/x".into()))
        );
    }

    #[test]
    fn split_url_keeps_ipv6_literal_intact() {
        assert_eq!(
            split_url("http://[::1]:8080/x"),
            Some(("[::1]".into(), "/x".into()))
        );
    }

    #[test]
    fn split_url_rejects_non_http_schemes() {
        assert_eq!(split_url("file:///etc/passwd"), None);
        assert_eq!(split_url("chrome://settings"), None);
    }

    // ----- Site matching -----------------------------------------------------

    #[test]
    fn site_pattern_matches_subdomains() {
        assert!(site_matches("www.reddit.com", "/", "reddit.com"));
        assert!(site_matches("old.reddit.com", "/r/x", "reddit.com"));
        assert!(site_matches("reddit.com", "/", "reddit.com"));
    }

    #[test]
    fn site_pattern_does_not_match_unrelated_suffix() {
        assert!(!site_matches("notreddit.com", "/", "reddit.com"));
        assert!(!site_matches("reddit.com.evil.test", "/", "reddit.com"));
    }

    #[test]
    fn site_pattern_path_acts_as_prefix() {
        assert!(site_matches(
            "reddit.com",
            "/r/rust/comments",
            "reddit.com/r/rust"
        ));
        assert!(!site_matches("reddit.com", "/r/cats", "reddit.com/r/rust"));
    }

    #[test]
    fn wildcard_and_www_prefixes_are_equivalent() {
        assert_eq!(
            normalize_site_pattern("*.example.com"),
            normalize_site_pattern("example.com")
        );
        assert_eq!(
            normalize_site_pattern("www.example.com"),
            normalize_site_pattern("example.com")
        );
        assert_eq!(
            normalize_site_pattern("https://example.com/"),
            normalize_site_pattern("example.com")
        );
    }

    #[test]
    fn bare_word_is_not_a_valid_site_pattern() {
        assert_eq!(normalize_site_pattern("reddit"), None);
    }

    // ----- Site decisions ----------------------------------------------------

    #[test]
    fn blocklist_blocks_only_listed_sites() {
        let rules = vec![rule("site", "block", "reddit.com", "hide")];
        assert_eq!(
            decide_site("https://reddit.com/r/x", &rules, false),
            SiteDecision::Blocked
        );
        assert_eq!(
            decide_site("https://docs.rs/serde", &rules, false),
            SiteDecision::Allowed
        );
    }

    #[test]
    fn strict_mode_blocks_everything_not_allowed() {
        let rules = vec![rule("site", "allow", "atlassian.net", "hide")];
        assert_eq!(
            decide_site("https://team.atlassian.net/browse/X-1", &rules, true),
            SiteDecision::Allowed
        );
        assert_eq!(
            decide_site("https://news.ycombinator.com", &rules, true),
            SiteDecision::Blocked
        );
    }

    #[test]
    fn allow_rule_carves_an_exception_out_of_a_block_rule() {
        let rules = vec![
            rule("site", "block", "reddit.com", "hide"),
            rule("site", "allow", "reddit.com/r/rust", "hide"),
        ];
        assert_eq!(
            decide_site("https://reddit.com/r/rust/top", &rules, false),
            SiteDecision::Allowed
        );
        assert_eq!(
            decide_site("https://reddit.com/r/aww", &rules, false),
            SiteDecision::Blocked
        );
    }

    #[test]
    fn strict_mode_is_inert_until_the_allow_list_has_an_entry() {
        // Flipping the switch before filling the list in must not hide every
        // app on the machine.
        let unknown = app("com.example.Whatever", "whatever.exe", "Whatever");
        assert_eq!(decide_app(&unknown, &[], true), AppDecision::Allowed);
        assert_eq!(
            decide_site("https://news.ycombinator.com", &[], true),
            SiteDecision::Allowed
        );
    }

    #[test]
    fn a_disabled_allow_rule_does_not_arm_strict_mode() {
        let mut allow = rule("app", "allow", "com.apple.Terminal", "hide");
        allow.enabled = false;
        let unknown = app("com.example.Whatever", "whatever.exe", "Whatever");
        assert_eq!(decide_app(&unknown, &[allow], true), AppDecision::Allowed);
    }

    #[test]
    fn an_allow_rule_of_the_other_kind_does_not_arm_strict_mode() {
        // A site allow-list says nothing about which apps are permitted.
        let rules = vec![rule("site", "allow", "atlassian.net", "hide")];
        let unknown = app("com.example.Whatever", "whatever.exe", "Whatever");
        assert_eq!(decide_app(&unknown, &rules, true), AppDecision::Allowed);
    }

    #[test]
    fn loopback_is_never_blocked_even_in_strict_mode() {
        let rules = vec![rule("site", "allow", "atlassian.net", "hide")];
        assert_eq!(
            decide_site("http://127.0.0.1:27420/blocked", &rules, true),
            SiteDecision::Allowed
        );
        assert_eq!(
            decide_site("http://localhost:1420/", &rules, true),
            SiteDecision::Allowed
        );
    }

    #[test]
    fn disabled_rules_are_ignored() {
        let mut r = rule("site", "block", "reddit.com", "hide");
        r.enabled = false;
        assert_eq!(
            decide_site("https://reddit.com", &[r], false),
            SiteDecision::Allowed
        );
    }

    // ----- App decisions -----------------------------------------------------

    #[test]
    fn app_rule_matches_bundle_exe_or_display_name() {
        let slack = app("com.slack.Slack", "slack.exe", "Slack");
        assert!(app_matches(&slack, "com.slack.slack"));
        assert!(app_matches(&slack, "slack.exe"));
        assert!(app_matches(&slack, "slack"));
        assert!(!app_matches(&slack, "discord"));
    }

    #[test]
    fn blocked_app_uses_its_configured_action() {
        let slack = app("com.slack.Slack", "slack.exe", "Slack");
        let rules = vec![rule("app", "block", "com.slack.Slack", "kill")];
        assert_eq!(
            decide_app(&slack, &rules, false),
            AppDecision::Blocked(AppAction::Kill)
        );
    }

    #[test]
    fn strict_mode_hides_but_never_kills() {
        let unknown = app("com.example.Whatever", "whatever.exe", "Whatever");
        let rules = vec![rule("app", "allow", "com.apple.Terminal", "kill")];
        assert_eq!(
            decide_app(&unknown, &rules, true),
            AppDecision::Blocked(AppAction::Hide)
        );
    }

    #[test]
    fn safe_list_apps_survive_strict_mode() {
        let rules = vec![rule("app", "allow", "com.example.Nothing", "hide")];
        for ident in [
            app("com.apple.finder", "", "Finder"),
            app("", "explorer.exe", "Windows Explorer"),
            app("com.tracker.app", "tracker.exe", "Tracker"),
        ] {
            assert_eq!(
                decide_app(&ident, &rules, true),
                AppDecision::Allowed,
                "{ident:?} must never be blocked"
            );
        }
    }

    #[test]
    fn safe_list_apps_survive_an_explicit_kill_rule() {
        let finder = app("com.apple.finder", "", "Finder");
        let rules = vec![rule("app", "block", "com.apple.finder", "kill")];
        assert_eq!(decide_app(&finder, &rules, false), AppDecision::Allowed);
    }

    #[test]
    fn app_allow_rule_beats_block_rule() {
        let slack = app("com.slack.Slack", "slack.exe", "Slack");
        let rules = vec![
            rule("app", "block", "slack", "kill"),
            rule("app", "allow", "com.slack.Slack", "hide"),
        ];
        assert_eq!(decide_app(&slack, &rules, false), AppDecision::Allowed);
    }
}

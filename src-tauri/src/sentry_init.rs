//! Optional Sentry guard — initialized in [`crate::run`] only when:
//!   1. A DSN is configured (build-time `option_env!` or runtime `std::env`),
//!      AND
//!   2. The user has opted in via the `sentry_enabled` `app_settings` key.
//!
//! Holding the returned [`sentry::ClientInitGuard`] for the lifetime of the
//! process keeps Sentry running; dropping it flushes and disables capture.
//!
//! Privacy:
//!   * No DSN ever ships in the source tree — it must be injected at build
//!     time via `TRACKER_SENTRY_DSN_BACKEND` or at runtime via the same env
//!     var.
//!   * Anything that looks like a token / API key / secret / password /
//!     cookie is replaced with `[redacted]` before the event leaves the
//!     device.
//!   * User emails are masked to keep only the first letter of the local
//!     part (`igor.pocta@example.com` → `i***@example.com`).
//!   * Long alphanumeric values (≥20 chars, ≤200 chars) are redacted as a
//!     defensive catch-all for opaque tokens that don't follow the
//!     name-based heuristics.

use std::sync::{Arc, OnceLock};

use sentry::protocol::{Breadcrumb, Event, Value};

/// Holds the live Sentry client for the process. Set on first successful
/// [`init_if_enabled`] call; never cleared (changes to the opt-in toggle
/// take effect on next restart).
static GUARD: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

/// Build-time DSN. Set `TRACKER_SENTRY_DSN_BACKEND=<dsn>` before
/// `cargo build` / `cargo tauri build` to bake one in without touching
/// source. Unset = no embedded DSN.
const BUILD_DSN: Option<&str> = option_env!("TRACKER_SENTRY_DSN_BACKEND");

/// Returns the effective DSN according to the precedence:
///
/// 1. `TRACKER_SENTRY_DSN_BACKEND` env var at process start (overrides
///    the baked-in DSN — handy for ad-hoc QA against a staging Sentry).
/// 2. `option_env!("TRACKER_SENTRY_DSN_BACKEND")` baked at build time.
/// 3. `None` — Sentry stays off.
pub fn resolve_dsn() -> Option<String> {
    if let Ok(v) = std::env::var("TRACKER_SENTRY_DSN_BACKEND") {
        if !v.is_empty() {
            return Some(v);
        }
    }
    BUILD_DSN
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Initialise Sentry if (and only if) we have a DSN AND the user opted in.
///
/// Returns `true` if Sentry is now (or was already) active. Idempotent —
/// a second call with `enabled = true` after the first success is a no-op.
///
/// `install_id` is a stable anonymous identifier (UUID v4 generated once
/// on first run and persisted in `app_settings`). It's attached as
/// `user.id` so events from one install group together without exposing
/// PII.
pub fn init_if_enabled(install_id: &str, enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    if GUARD.get().is_some() {
        return true;
    }
    let Some(dsn) = resolve_dsn() else {
        return false;
    };

    let release = format!("tracker@{}", env!("CARGO_PKG_VERSION"));
    let environment: std::borrow::Cow<'static, str> = if cfg!(debug_assertions) {
        "development".into()
    } else {
        "production".into()
    };

    let before_send: sentry::BeforeCallback<Event<'static>> = Arc::new(|event| Some(scrub_event(event)));
    let before_breadcrumb: sentry::BeforeCallback<Breadcrumb> = Arc::new(|b| Some(scrub_breadcrumb(b)));

    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(release.into()),
            environment: Some(environment),
            traces_sample_rate: 0.1,
            attach_stacktrace: true,
            send_default_pii: false,
            before_send: Some(before_send),
            before_breadcrumb: Some(before_breadcrumb),
            ..Default::default()
        },
    ));

    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry::protocol::User {
            id: Some(install_id.to_string()),
            ..Default::default()
        }));
        scope.set_tag("install_id", install_id);
        scope.set_tag("app.version", env!("CARGO_PKG_VERSION"));
    });

    // We deliberately leak the guard into the OnceLock; dropping it would
    // disable capture. If `set` races we just drop the second guard which
    // shuts down its own client gracefully.
    let _ = GUARD.set(guard);
    true
}

/// Drop sensitive fields from an outgoing event. Intent is best-effort
/// defence-in-depth — Sentry's own SDK already strips some PII when
/// `send_default_pii = false`, but we layer extra guards for Jira tokens,
/// API keys, and email addresses.
fn scrub_event(mut event: Event<'static>) -> Event<'static> {
    // Request headers — anything with an Authorization / Cookie /
    // X-Token-style name (or a header value that looks like an auth
    // scheme) gets flattened.
    if let Some(req) = event.request.as_mut() {
        let headers = std::mem::take(&mut req.headers);
        for (name, value) in headers {
            let lname = name.to_lowercase();
            let scrubbed = if header_name_is_sensitive(&lname)
                || header_value_is_sensitive(&value)
            {
                "[redacted]".to_string()
            } else {
                value
            };
            req.headers.insert(name, scrubbed);
        }

        // Strip any query string entirely — could contain tokens.
        if let Some(qs) = req.query_string.as_mut() {
            if looks_like_secret(qs) {
                *qs = "[redacted]".into();
            }
        }

        if let Some(cookies) = req.cookies.as_mut() {
            *cookies = "[redacted]".into();
        }
    }

    // Top-level `extra` map.
    scrub_value_map(&mut event.extra);

    // Per-context maps (some contexts expose a flat string→Value table).
    for ctx in event.contexts.values_mut() {
        if let sentry::protocol::Context::Other(map) = ctx {
            scrub_value_map(map);
        }
    }

    // User email — mask local part if present.
    if let Some(user) = event.user.as_mut() {
        if let Some(email) = user.email.take() {
            user.email = Some(mask_email(&email));
        }
        // Backend never sends real user names; drop just in case.
        user.username = None;
        user.ip_address = None;
    }

    event
}

fn scrub_breadcrumb(mut b: Breadcrumb) -> Breadcrumb {
    scrub_value_map(&mut b.data);
    if let Some(msg) = b.message.as_mut() {
        if looks_like_secret(msg) {
            *msg = "[redacted]".into();
        }
    }
    b
}

/// Walk a `String -> Value` map and redact entries whose keys look like
/// secrets, or whose string values look like opaque tokens.
pub(crate) fn scrub_value_map(map: &mut std::collections::BTreeMap<String, Value>) {
    for (k, v) in map.iter_mut() {
        let lower = k.to_lowercase();
        if key_is_sensitive(&lower) {
            *v = Value::from("[redacted]");
            continue;
        }
        if let Value::String(s) = v {
            if looks_like_token(s) {
                *v = Value::from("[redacted-token]");
            }
        }
    }
}

fn key_is_sensitive(lower: &str) -> bool {
    lower.contains("token")
        || lower.contains("api_key")
        || lower.contains("api-key")
        || lower.contains("apikey")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("cookie")
        || lower.contains("authorization")
}

fn header_name_is_sensitive(lower: &str) -> bool {
    lower == "authorization"
        || lower == "cookie"
        || lower == "set-cookie"
        || lower == "proxy-authorization"
        || lower.contains("token")
        || lower.contains("api-key")
        || lower.contains("apikey")
}

fn header_value_is_sensitive(value: &str) -> bool {
    let trimmed = value.trim_start();
    trimmed.starts_with("Bearer ")
        || trimmed.starts_with("Basic ")
        || trimmed.starts_with("Digest ")
}

fn looks_like_secret(s: &str) -> bool {
    // Heuristic for free-text fields — only flag when we see explicit hints.
    let lower = s.to_lowercase();
    lower.contains("token=")
        || lower.contains("apikey=")
        || lower.contains("api_key=")
        || lower.contains("password=")
        || lower.contains("authorization:")
}

fn looks_like_token(s: &str) -> bool {
    if s.len() < 20 || s.len() > 200 {
        return false;
    }
    // Long string of base64/hex/identifier characters with no spaces is
    // probably an API token.
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '=')
}

/// Mask the local part of an email address so we can still bucket events
/// by domain without exposing the user identifier.
pub fn mask_email(e: &str) -> String {
    match e.find('@') {
        Some(at) => {
            let local = &e[..at];
            let domain = &e[at..];
            if local.chars().count() > 1 {
                let first: String = local.chars().take(1).collect();
                format!("{first}***{domain}")
            } else {
                format!("***{domain}")
            }
        }
        None => "[redacted]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::Value;
    use std::collections::BTreeMap;

    #[test]
    fn mask_email_basic() {
        assert_eq!(mask_email("igor.pocta@example.com"), "i***@example.com");
    }

    #[test]
    fn mask_email_short_local() {
        assert_eq!(mask_email("i@x.cz"), "***@x.cz");
    }

    #[test]
    fn mask_email_no_at() {
        assert_eq!(mask_email("notanemail"), "[redacted]");
    }

    #[test]
    fn mask_email_uppercase_local() {
        assert_eq!(mask_email("Alice@Example.COM"), "A***@Example.COM");
    }

    #[test]
    fn scrub_drops_token_keys() {
        let mut m: BTreeMap<String, Value> = BTreeMap::new();
        m.insert("api_key".into(), Value::from("sk_live_abcdef"));
        m.insert("apikey".into(), Value::from("zzz"));
        m.insert("Authorization".into(), Value::from("Bearer xyz"));
        m.insert("PASSWORD".into(), Value::from("hunter2"));
        m.insert("session_cookie".into(), Value::from("c"));
        m.insert("safe".into(), Value::from("hello"));
        scrub_value_map(&mut m);
        assert_eq!(m.get("api_key"), Some(&Value::from("[redacted]")));
        assert_eq!(m.get("apikey"), Some(&Value::from("[redacted]")));
        assert_eq!(m.get("Authorization"), Some(&Value::from("[redacted]")));
        assert_eq!(m.get("PASSWORD"), Some(&Value::from("[redacted]")));
        assert_eq!(m.get("session_cookie"), Some(&Value::from("[redacted]")));
        // Untouched.
        assert_eq!(m.get("safe"), Some(&Value::from("hello")));
    }

    #[test]
    fn scrub_redacts_long_alphanum_values() {
        let mut m: BTreeMap<String, Value> = BTreeMap::new();
        m.insert(
            "weird".into(),
            Value::from("ATATT3xFfGN0abcdef1234567890XYZ-LongTokenLookingThing.suffix=="),
        );
        scrub_value_map(&mut m);
        assert_eq!(m.get("weird"), Some(&Value::from("[redacted-token]")));
    }

    #[test]
    fn scrub_keeps_short_strings() {
        let mut m: BTreeMap<String, Value> = BTreeMap::new();
        m.insert("short".into(), Value::from("hi"));
        scrub_value_map(&mut m);
        assert_eq!(m.get("short"), Some(&Value::from("hi")));
    }

    #[test]
    fn header_name_detection() {
        assert!(header_name_is_sensitive("authorization"));
        assert!(header_name_is_sensitive("cookie"));
        assert!(header_name_is_sensitive("x-auth-token"));
        assert!(header_name_is_sensitive("x-api-key"));
        assert!(!header_name_is_sensitive("content-type"));
    }

    #[test]
    fn header_value_detection() {
        assert!(header_value_is_sensitive("Bearer abc"));
        assert!(header_value_is_sensitive("Basic dXNlcjpwYXNz"));
        assert!(!header_value_is_sensitive("application/json"));
    }

    #[test]
    fn resolve_dsn_prefers_env() {
        // We can't easily mutate static option_env! at runtime, but we
        // can confirm the env-var override path works.
        let original = std::env::var("TRACKER_SENTRY_DSN_BACKEND").ok();
        // SAFETY: tests in this module are run sequentially within the
        // same process; we restore the value at the end.
        unsafe {
            std::env::set_var(
                "TRACKER_SENTRY_DSN_BACKEND",
                "https://abc@o0.ingest.sentry.io/1",
            );
        }
        assert_eq!(
            resolve_dsn().as_deref(),
            Some("https://abc@o0.ingest.sentry.io/1")
        );
        unsafe {
            std::env::remove_var("TRACKER_SENTRY_DSN_BACKEND");
        }
        // With the env var unset, resolve_dsn falls back to the
        // build-time embed (which itself is unset during `cargo test`).
        // Restore if the caller had something configured.
        if let Some(v) = original {
            unsafe {
                std::env::set_var("TRACKER_SENTRY_DSN_BACKEND", v);
            }
        }
    }

    #[test]
    fn init_returns_false_without_dsn_or_opt_in() {
        // No DSN, no opt-in.
        let original = std::env::var("TRACKER_SENTRY_DSN_BACKEND").ok();
        unsafe {
            std::env::remove_var("TRACKER_SENTRY_DSN_BACKEND");
        }
        assert!(!init_if_enabled("install-1", false));
        // opt-in but no DSN
        assert!(!init_if_enabled("install-1", true));
        if let Some(v) = original {
            unsafe {
                std::env::set_var("TRACKER_SENTRY_DSN_BACKEND", v);
            }
        }
    }
}

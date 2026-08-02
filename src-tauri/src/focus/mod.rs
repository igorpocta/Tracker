//! Focus mode — block distracting apps and websites for the duration of a
//! focused work session.
//!
//! ```text
//!   engine   ── ticks once a second while a session runs
//!     ├── apps      frontmost app + hide/terminate
//!     ├── browsers  macOS AppleScript URL read/rewrite (Safari, Chromium)
//!     ├── notify    system Do Not Disturb
//!     ├── overlay   the "this app is blocked" banner
//!     └── rules     pure decision logic (no I/O — see its tests)
//! ```
//!
//! Other browsers are handled out-of-process by the Tracker Bridge extension,
//! which pulls the ruleset from `GET /focus/state` and installs
//! `declarativeNetRequest` rules. Both paths redirect to the same block page
//! served by [`crate::server`].
//!
//! The design and its trade-offs are written up in
//! `docs/plans/2026-08-02-focus-mode-design.md`.

pub mod apps;
pub mod browsers;
pub mod engine;
pub mod notify;
pub mod overlay;
pub mod rules;

/// Percent-encode everything outside the unreserved set, so an arbitrary URL
/// survives being carried in the `u=` query parameter of the block page.
///
/// Hand-rolled rather than pulling in a dependency: this is the only place
/// that needs it, and the rule is three lines.
pub fn percent_encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Reverse of [`percent_encode`]. Invalid escapes are passed through verbatim
/// rather than dropped — the value is only ever displayed, and mangling it
/// further would just confuse the user.
pub fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// URL of the local block page for `original`.
pub fn blocked_page_url(original: &str) -> String {
    format!(
        "http://127.0.0.1:{}/blocked?u={}",
        crate::server::SERVER_PORT,
        percent_encode(original)
    )
}

/// Is this URL already the block page? Prevents an enforcement loop where we
/// keep redirecting our own redirect.
pub fn is_blocked_page(url: &str) -> bool {
    let prefix_v4 = format!("http://127.0.0.1:{}/blocked", crate::server::SERVER_PORT);
    let prefix_local = format!("http://localhost:{}/blocked", crate::server::SERVER_PORT);
    url.starts_with(&prefix_v4) || url.starts_with(&prefix_local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips_a_messy_url() {
        let url = "https://example.com/a b?x=1&y=č#frag";
        assert_eq!(percent_decode(&percent_encode(url)), url);
    }

    #[test]
    fn encoding_escapes_query_delimiters() {
        let encoded = percent_encode("a?b&c=d");
        assert!(!encoded.contains('?'));
        assert!(!encoded.contains('&'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn decoding_leaves_a_truncated_escape_alone() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn block_page_url_is_recognised_as_such() {
        let url = blocked_page_url("https://reddit.com");
        assert!(is_blocked_page(&url));
        assert!(!is_blocked_page("https://reddit.com"));
    }
}

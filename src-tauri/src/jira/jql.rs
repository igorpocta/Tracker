//! JQL helpers.

/// Default JQL used by the full sync.
///
/// We deliberately do **not** filter by assignee here — the user expects
/// "all my reachable issues" to be cached, not just the ones currently
/// assigned to them. Returns everything visible to the configured account,
/// ordered most-recently-updated first; the per-page `maxResults` cap +
/// [`SYNC_MAX_RESULTS_TOTAL`] act as the real upper bound.
pub const DEFAULT_JQL: &str = r#"ORDER BY updated DESC"#;

/// Per-page issue cap requested from `/search/jql`. Jira will return at most
/// this many issues per HTTP call.
pub const SYNC_PAGE_MAX_RESULTS: u32 = 100;

/// Safety cap on the **total** issues pulled across all pages by a single
/// `sync_issues_from_jira` invocation. Keeps an accidental "ORDER BY updated"
/// against a huge instance from looping forever / filling the disk.
pub const SYNC_MAX_RESULTS_TOTAL: usize = 5_000;

/// Escape a string for safe inclusion inside a JQL double-quoted literal.
///
/// Useful when programmatically constructing JQL like `summary ~ "<user input>"`.
pub fn escape_quoted(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

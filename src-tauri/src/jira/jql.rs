//! JQL helpers.

/// Default JQL used by the full sync. Pulls everything that's still relevant:
/// either not done, or done but updated within the last two weeks.
pub const DEFAULT_JQL: &str =
    r#"NOT (statusCategory = "Done" AND updated < "-14d") ORDER BY updated DESC"#;

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

//! JQL helpers.

/// Default JQL used by the full sync.
///
/// Atlassian Cloud od května 2026 odmítá "neomezené" JQL dotazy bez aspoň
/// jedné restrikce — vrací 400 s textem "Dotazy JQL bez limitů tu nejsou
/// povoleny." Náš původní `ORDER BY updated DESC` proto přestal fungovat.
///
/// Filtr `project IS NOT EMPTY` projde validátorem a vrátí vše, co
/// uživatel v Jiře vidí (Atlassian ACL si pohlídá viditelnost sám). To je
/// nutné, aby uživatel mohl trackovat i úkoly, na kterých ještě
/// nepracoval — bez tohoto by `issues_v2` obsahovala jen vlastní /
/// nalogované úkoly a vyhledávač by zbytek nikdy neukazoval.
///
/// Strop `SYNC_MAX_RESULTS_TOTAL` chrání před opravdu monstrózními
/// instancemi; uživatelé s 10 000+ ticketů si můžou nastavit vlastní
/// `sync_jql` v connection configu (např. omezit na konkrétní projekty).
pub const DEFAULT_JQL: &str = r#"project IS NOT EMPTY ORDER BY updated DESC"#;

/// Per-page issue cap requested from `/search/jql`. Jira will return at most
/// this many issues per HTTP call.
pub const SYNC_PAGE_MAX_RESULTS: u32 = 100;

/// Safety cap on the **total** issues pulled across all pages by a single
/// `sync_issues_from_jira` invocation. Keeps an accidental "ORDER BY updated"
/// against a huge instance from looping forever / filling the disk.
pub const SYNC_MAX_RESULTS_TOTAL: usize = 20_000;

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

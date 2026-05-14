//! Input validation helpers shared by Tauri commands.
//!
//! Phase 18C — Item 23. Centralises the "is this Jira-key/JQL/numeric input
//! actually safe to pass into a `Result<_, String>`-returning command?" checks
//! so they're consistent (and Czech) across the surface.
//!
//! The functions here return `Result<(), String>` so the call site can `?` the
//! error directly. Error messages are user-facing and Czech.

/// Maximum length of a Jira issue key. Atlassian's documented maximum is much
/// higher in theory, but in practice projects use 2–4 letter codes plus
/// number — 64 chars is generous.
pub const MAX_ISSUE_KEY_LEN: usize = 64;

/// Maximum length of a JQL query we'll forward to Jira. Anything beyond this
/// is almost certainly a paste error or someone trying to exploit the input.
pub const MAX_JQL_LEN: usize = 2000;

/// Validate that `key` is a syntactically plausible Jira issue key
/// (`^[A-Z][A-Z0-9]+-[1-9][0-9]*$`). Hand-rolled so we don't pull in `regex`
/// for a single check.
///
/// Rules:
///   - Must contain exactly one `-`.
///   - Project part: starts with `A..Z`, length ≥ 2, only `A..Z` / `0..9`.
///   - Number part: starts with `1..9`, only digits, fits in `i64`.
///
/// Empty / whitespace / lowercase / leading-zero are rejected.
pub fn validate_issue_key(key: &str) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("Klíč úkolu nesmí být prázdný".into());
    }
    if trimmed.len() > MAX_ISSUE_KEY_LEN {
        return Err(format!(
            "Klíč úkolu je příliš dlouhý (max {MAX_ISSUE_KEY_LEN} znaků)"
        ));
    }
    let mut parts = trimmed.splitn(2, '-');
    let proj = parts.next().unwrap_or("");
    let num = parts.next();
    let num = match num {
        Some(n) => n,
        None => return Err(format!("Neplatný klíč úkolu {trimmed:?} (chybí pomlčka)")),
    };
    if trimmed.matches('-').count() != 1 {
        return Err(format!("Neplatný klíč úkolu {trimmed:?} (více pomlček)"));
    }
    if proj.len() < 2 {
        return Err(format!("Neplatný klíč úkolu {trimmed:?} (krátká zkratka)"));
    }
    let mut chars = proj.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_uppercase() {
        return Err(format!(
            "Neplatný klíč úkolu {trimmed:?} (zkratka musí začínat velkým písmenem)"
        ));
    }
    for c in chars {
        if !(c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return Err(format!(
                "Neplatný klíč úkolu {trimmed:?} (znak {c:?} ve zkratce)"
            ));
        }
    }
    if num.is_empty() {
        return Err(format!("Neplatný klíč úkolu {trimmed:?} (chybí číslo)"));
    }
    let mut num_chars = num.chars();
    let first_n = num_chars.next().unwrap();
    if !('1'..='9').contains(&first_n) {
        return Err(format!(
            "Neplatný klíč úkolu {trimmed:?} (číslo nesmí začínat nulou)"
        ));
    }
    for c in num_chars {
        if !c.is_ascii_digit() {
            return Err(format!(
                "Neplatný klíč úkolu {trimmed:?} (znak {c:?} v čísle)"
            ));
        }
    }
    if num.parse::<i64>().is_err() {
        return Err(format!("Neplatný klíč úkolu {trimmed:?} (číslo přetéká)"));
    }
    Ok(())
}

/// Reject embedded NUL bytes and over-length inputs. Used for free-text
/// fields like JQL and comments, where a NUL would silently truncate the
/// string in some SQLite/JSON paths.
pub fn validate_free_text(input: &str, max_len: usize, field: &str) -> Result<(), String> {
    if input.contains('\0') {
        return Err(format!("{field} obsahuje neplatný znak (NUL)"));
    }
    if input.len() > max_len {
        return Err(format!(
            "{field} je příliš dlouhý (max {max_len} znaků, zadáno {})",
            input.len()
        ));
    }
    Ok(())
}

/// Validate a JQL query: non-empty (after trim), within length budget, no
/// NUL bytes.
pub fn validate_jql(jql: &str) -> Result<(), String> {
    if jql.trim().is_empty() {
        return Err("JQL dotaz nesmí být prázdný".into());
    }
    validate_free_text(jql, MAX_JQL_LEN, "JQL dotaz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_key_accepts_canonical() {
        for k in ["ACME-1", "ACME-12345", "AB-9", "ABC123-7", "PROJ-1000000"] {
            assert!(validate_issue_key(k).is_ok(), "{k} should be accepted");
        }
    }

    #[test]
    fn issue_key_trims_whitespace() {
        assert!(validate_issue_key("  ACME-1  ").is_ok());
    }

    #[test]
    fn issue_key_rejects_empty() {
        assert!(validate_issue_key("").is_err());
        assert!(validate_issue_key("   ").is_err());
    }

    #[test]
    fn issue_key_rejects_lowercase() {
        assert!(validate_issue_key("acme-1").is_err());
        assert!(validate_issue_key("Acme-1").is_err());
    }

    #[test]
    fn issue_key_rejects_no_hyphen() {
        assert!(validate_issue_key("ACME1").is_err());
        assert!(validate_issue_key("ACME").is_err());
    }

    #[test]
    fn issue_key_rejects_multiple_hyphens() {
        assert!(validate_issue_key("ACME-1-2").is_err());
    }

    #[test]
    fn issue_key_rejects_leading_zero() {
        assert!(validate_issue_key("ACME-01").is_err());
        assert!(validate_issue_key("ACME-0").is_err());
    }

    #[test]
    fn issue_key_rejects_short_project() {
        assert!(validate_issue_key("A-1").is_err());
    }

    #[test]
    fn issue_key_rejects_special_chars() {
        assert!(validate_issue_key("AC ME-1").is_err());
        assert!(validate_issue_key("ACME_1-1").is_err());
        assert!(validate_issue_key("ACME-1a").is_err());
        assert!(validate_issue_key("ACME-1.5").is_err());
    }

    #[test]
    fn issue_key_rejects_overlong() {
        let key = format!("{}-1", "A".repeat(MAX_ISSUE_KEY_LEN));
        assert!(validate_issue_key(&key).is_err());
    }

    #[test]
    fn jql_rejects_empty() {
        assert!(validate_jql("").is_err());
        assert!(validate_jql("   \t\n  ").is_err());
    }

    #[test]
    fn jql_accepts_typical_queries() {
        assert!(validate_jql("project = ACME").is_ok());
        assert!(validate_jql("assignee = currentUser() AND status != Done").is_ok());
    }

    #[test]
    fn jql_rejects_nul() {
        let bad = "project = ACME\0; DROP TABLE".to_string();
        assert!(validate_jql(&bad).is_err());
    }

    #[test]
    fn jql_rejects_overlong() {
        let long = "x".repeat(MAX_JQL_LEN + 1);
        assert!(validate_jql(&long).is_err());
    }

    #[test]
    fn free_text_rejects_nul_and_length() {
        assert!(validate_free_text("abc", 5, "Pole").is_ok());
        assert!(validate_free_text("a\0b", 5, "Pole").is_err());
        assert!(validate_free_text("longer than", 5, "Pole").is_err());
    }
}

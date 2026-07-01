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

/// Provider-agnostic safety minimum for a provider base URL:
///   - scheme must be `https` (a debug build tolerates `http` for local dev),
///   - no embedded credentials (`user:pass@host`),
///   - host must not be `localhost`, nor a loopback / private / link-local /
///     multicast / unspecified IP (loopback tolerated only in debug builds).
///
/// Stops a compromised renderer or an imported backup config from pointing a
/// connection — and thus its API token — at an arbitrary internal host
/// (token exfiltration / SSRF).
pub fn validate_base_url_safety(base_url: &str) -> Result<(), String> {
    let url = url::Url::parse(base_url).map_err(|e| format!("neplatná URL: {e}"))?;

    // Local dev over http/loopback is only ever allowed in a debug build, never
    // in a release binary — matches the "no implicit dev exceptions" rule.
    let allow_local = cfg!(debug_assertions);

    match url.scheme() {
        "https" => {}
        "http" if allow_local => {}
        other => return Err(format!("URL musí používat https (dostala „{other}“)")),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL nesmí obsahovat přihlašovací údaje".to_string());
    }

    match url.host() {
        Some(url::Host::Domain(d)) => {
            let host = d.to_ascii_lowercase();
            if host.is_empty() {
                return Err("URL nemá host".to_string());
            }
            if host == "localhost" && !allow_local {
                return Err("localhost není povolen mimo dev build".to_string());
            }
        }
        Some(url::Host::Ipv4(ip)) => {
            let dangerous = ip.is_private()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || (ip.is_loopback() && !allow_local);
            if dangerous {
                return Err(format!("IP adresa {ip} není povolena"));
            }
        }
        Some(url::Host::Ipv6(ip)) => {
            // fc00::/7 (unique-local) a fe80::/10 (link-local) nejsou ve stable
            // API, počítáme je ručně ze segmentů.
            let ula = (ip.segments()[0] & 0xfe00) == 0xfc00;
            let link_local = (ip.segments()[0] & 0xffc0) == 0xfe80;
            let dangerous = ip.is_multicast()
                || ip.is_unspecified()
                || ula
                || link_local
                || (ip.is_loopback() && !allow_local);
            if dangerous {
                return Err(format!("IP adresa {ip} není povolena"));
            }
        }
        None => return Err("URL nemá host".to_string()),
    }

    Ok(())
}

/// Full base-URL policy for a specific provider: the safety minimum above plus
/// a host allow-list. Freelo must target `freelo.io`; Jira defaults to Atlassian
/// Cloud (`*.atlassian.net`) and only accepts another host when the connection
/// explicitly opts into a custom (self-hosted) mode — still over HTTPS.
pub fn validate_provider_base_url(
    provider: &str,
    base_url: &str,
    allow_custom_host: bool,
) -> Result<(), String> {
    validate_base_url_safety(base_url)?;
    let url = url::Url::parse(base_url).map_err(|e| format!("neplatná URL: {e}"))?;

    // Dev exception: a debug build may point a connection at a local mock
    // (loopback / localhost). Safety already permitted it above; also let it
    // bypass the SaaS host allow-list so integration tests can run. Never in
    // release — production always enforces the allow-list.
    if cfg!(debug_assertions) {
        match url.host() {
            Some(url::Host::Domain(d)) if d.eq_ignore_ascii_case("localhost") => return Ok(()),
            Some(url::Host::Ipv4(ip)) if ip.is_loopback() => return Ok(()),
            Some(url::Host::Ipv6(ip)) if ip.is_loopback() => return Ok(()),
            _ => {}
        }
    }

    let host = url.host_str().unwrap_or("").to_ascii_lowercase();

    match provider {
        "freelo" => {
            let is_freelo = host == "freelo.io" || host.ends_with(".freelo.io");
            if !is_freelo {
                return Err("Freelo připojení musí mířit na freelo.io".to_string());
            }
        }
        "jira" => {
            let is_cloud = host == "atlassian.net" || host.ends_with(".atlassian.net");
            if !is_cloud && !allow_custom_host {
                return Err(
                    "Neznámý Jira host — pro self-hosted Jira povolte custom režim".to_string(),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_key_accepts_canonical() {
        for k in [
            "ACME-1",
            "ACME-12345",
            "AB-9",
            "ABC123-7",
            "PROJ-1000000",
            // Phase 18E: Freelo synthetic keys (`FRL-{task_id}`) must also
            // pass — the backend dispatches by prefix downstream.
            "FRL-1",
            "FRL-99999",
        ] {
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

    #[test]
    fn base_url_safety_rejects_dangerous_targets() {
        // scheme
        assert!(validate_base_url_safety("ftp://x.atlassian.net").is_err());
        // credentials in URL
        assert!(validate_base_url_safety("https://user:pass@x.atlassian.net").is_err());
        // private / link-local / multicast / ULA IPs are rejected in ALL builds.
        // (loopback + localhost are debug-only dev exceptions, so not asserted
        // here — a debug test build would accept them by design.)
        assert!(validate_base_url_safety("https://10.0.0.5").is_err());
        assert!(validate_base_url_safety("https://192.168.1.10").is_err());
        assert!(validate_base_url_safety("https://169.254.1.1").is_err());
        assert!(validate_base_url_safety("https://224.0.0.1").is_err());
        assert!(validate_base_url_safety("https://[fe80::1]").is_err());
        assert!(validate_base_url_safety("https://[fc00::1]").is_err());
    }

    #[test]
    fn base_url_safety_accepts_public_https() {
        assert!(validate_base_url_safety("https://firma.atlassian.net").is_ok());
        assert!(validate_base_url_safety("https://api.freelo.io/v1").is_ok());
        assert!(validate_base_url_safety("https://jira.example.com").is_ok());
    }

    #[test]
    fn provider_url_enforces_host_allowlist() {
        // Jira Cloud host is fine without custom mode.
        assert!(validate_provider_base_url("jira", "https://firma.atlassian.net", false).is_ok());
        // Unknown Jira host requires the explicit custom opt-in.
        assert!(validate_provider_base_url("jira", "https://jira.evil.com", false).is_err());
        assert!(validate_provider_base_url("jira", "https://jira.selfhosted.com", true).is_ok());
        // Freelo must target freelo.io; no custom mode.
        assert!(validate_provider_base_url("freelo", "https://api.freelo.io/v1", false).is_ok());
        assert!(validate_provider_base_url("freelo", "https://evil.com", true).is_err());
        // A safety failure (private IP) is rejected regardless of allow-list.
        assert!(validate_provider_base_url("jira", "https://10.0.0.1", true).is_err());
        // Dev exception (debug test build): loopback bypasses the allow-list so
        // integration tests can point at a local mock.
        assert!(validate_provider_base_url("jira", "http://127.0.0.1:8080", false).is_ok());
    }
}

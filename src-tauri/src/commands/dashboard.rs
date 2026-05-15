//! Jira Dashboard — agreguje úkoly napříč všemi Jira připojeními, která mají
//! v configu zapnuté `dashboard_enabled` a vyplněné `dashboard_jql`. Vrací
//! flat DTO se sloupci, které UI tabulka přímo renderuje (úkol, pověřená
//! osoba, zadavatel, priorita, stav, vytvořeno, termín dokončení).
//!
//! Sync není potřeba — command se ptá Jiry každým voláním přímo, protože
//! `issues_v2` neukládá většinu těch polí (avatar URLs, due date, reporter).
//! Pro typický pracovní seznam (10–500 úkolů) je to jeden API hop per
//! connection a celkový čas je v desítkách až stovkách ms.

use serde::{Deserialize, Serialize};

use crate::commands::connections::JiraConnectionConfig;
use crate::state::{AppState, ProviderClient};

/// Jeden řádek dashboardu — flat shape pro frontend tabulku.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraDashboardRow {
    pub connection_id: i64,
    pub connection_name: String,
    /// Atlassian base URL pro tu connection — UI z toho skládá `…/browse/KEY`.
    pub base_url: String,
    pub issue_key: String,
    pub summary: String,
    pub assignee: Option<JiraDashboardPerson>,
    pub reporter: Option<JiraDashboardPerson>,
    pub priority: Option<String>,
    pub status: Option<String>,
    /// Status category key (`new`/`indeterminate`/`done`) — UI z něj odvodí
    /// barvu badge.
    pub status_category: Option<String>,
    pub created: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraDashboardPerson {
    pub display_name: String,
    pub account_id: Option<String>,
    pub email: Option<String>,
    /// Nejvyšší dostupná velikost z `avatarUrls` (48 → 32 → 24 → 16). `None`
    /// pro neznámé / nepřítomné avatary; FE pak zobrazí iniciálu.
    pub avatar_url: Option<String>,
}

/// Fields předané do `search_jql`. Mimo úzkého syncu vůbec nesahá na
/// `issues_v2` cache, takže můžeme tahat širší set než `SYNC_FIELDS`.
const DASHBOARD_FIELDS: &[&str] = &[
    "summary", "status", "priority", "assignee", "reporter", "created", "duedate",
];

const PAGE_SIZE: u32 = 100;
/// Tvrdý strop, aby JQL bez `LIMIT`-style filtru nestáhlo desítky tisíc
/// úkolů. Když uživatel chce víc, řeší to přes přesnější JQL.
const MAX_ROWS_PER_CONNECTION: usize = 1_000;

/// `get_jira_dashboard_issues()` — agregace přes všechny enabled Jira
/// connections s `dashboard_enabled = true`.
///
/// Per-connection chyby se neeskalují — UI dostane částečné výsledky a
/// případné chybové texty per connection může zobrazit zvlášť (viz
/// `errors` v návratovém typu).
#[tauri::command]
pub async fn get_jira_dashboard_issues(
    state: tauri::State<'_, AppState>,
) -> Result<JiraDashboardResponse, String> {
    let active = state.connections.read().unwrap().clone();
    let mut rows: Vec<JiraDashboardRow> = Vec::new();
    let mut errors: Vec<JiraDashboardError> = Vec::new();

    for conn in active {
        let client = match &conn.client {
            ProviderClient::Jira(c) => c.clone(),
            _ => continue,
        };
        // Načti config a opt-in flag.
        let row =
            match cache::connections::get_by_id(&state.db, conn.id).map_err(|e| e.to_string())? {
                Some(r) => r,
                None => continue,
            };
        let cfg: JiraConnectionConfig = serde_json::from_str(&row.config_json).unwrap_or_default();
        if !cfg.dashboard_enabled {
            continue;
        }
        let jql = match cfg.dashboard_jql.as_deref().map(str::trim) {
            Some(j) if !j.is_empty() => j.to_string(),
            _ => continue,
        };

        match fetch_one_connection(&client, &conn.name, conn.id, &jql).await {
            Ok(mut r) => rows.append(&mut r),
            Err(e) => errors.push(JiraDashboardError {
                connection_id: conn.id,
                connection_name: conn.name.clone(),
                error: e,
            }),
        }
    }

    Ok(JiraDashboardResponse { rows, errors })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraDashboardResponse {
    pub rows: Vec<JiraDashboardRow>,
    pub errors: Vec<JiraDashboardError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraDashboardError {
    pub connection_id: i64,
    pub connection_name: String,
    pub error: String,
}

async fn fetch_one_connection(
    client: &crate::jira::JiraClient,
    conn_name: &str,
    conn_id: i64,
    jql: &str,
) -> Result<Vec<JiraDashboardRow>, String> {
    let base_url = client.base_url().to_string();
    let mut out: Vec<JiraDashboardRow> = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let page = client
            .search_jql(jql, page_token.as_deref(), DASHBOARD_FIELDS, PAGE_SIZE)
            .await
            .map_err(|e| e.to_string())?;
        for issue in &page.issues {
            out.push(map_issue_to_dashboard_row(
                issue, conn_id, conn_name, &base_url,
            ));
            if out.len() >= MAX_ROWS_PER_CONNECTION {
                return Ok(out);
            }
        }
        if page.is_last || page.next_page_token.is_none() {
            break;
        }
        page_token = page.next_page_token;
    }
    Ok(out)
}

fn map_issue_to_dashboard_row(
    issue: &crate::jira::JiraIssue,
    connection_id: i64,
    connection_name: &str,
    base_url: &str,
) -> JiraDashboardRow {
    let f = &issue.fields;
    JiraDashboardRow {
        connection_id,
        connection_name: connection_name.to_string(),
        base_url: base_url.to_string(),
        issue_key: issue.key.clone(),
        summary: f.summary.clone().unwrap_or_default(),
        assignee: map_person(f.assignee.as_ref()),
        reporter: map_person(f.reporter.as_ref()),
        priority: f.priority.as_ref().and_then(|p| p.name.clone()),
        status: f.status.as_ref().and_then(|s| s.name.clone()),
        status_category: f
            .status
            .as_ref()
            .and_then(|s| s.status_category.as_ref())
            .and_then(|c| c.key.clone()),
        created: f.created.clone(),
        due_date: f.duedate.clone(),
    }
}

fn map_person(p: Option<&crate::jira::models::JiraAssignee>) -> Option<JiraDashboardPerson> {
    let p = p?;
    let display = p
        .display_name
        .clone()
        .or_else(|| p.email_address.clone())
        .or_else(|| p.account_id.clone())?;
    // Vyber největší avatar, který Jira poslala — UI ho použije pro <img>.
    let avatar = p.avatar_urls.as_ref().and_then(|a| {
        a.size_48
            .clone()
            .or_else(|| a.size_32.clone())
            .or_else(|| a.size_24.clone())
            .or_else(|| a.size_16.clone())
    });
    Some(JiraDashboardPerson {
        display_name: display,
        account_id: p.account_id.clone(),
        email: p.email_address.clone(),
        avatar_url: avatar,
    })
}

use crate::cache;

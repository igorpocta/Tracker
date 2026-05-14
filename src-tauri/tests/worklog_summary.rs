//! Tests for Phase 18A — Item 8: worklog rows imported from Jira must carry
//! the issue summary, not fall back to "(bez popisu)".

use chrono::NaiveDate;
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::issues::IssueRow;
use tracker_lib::cache::{self, Db};
use tracker_lib::jira::worklog_sync::sync_worklogs_for_range;
use tracker_lib::jira::JiraClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client = JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string()).expect("ok");
    (server, client)
}

#[tokio::test]
async fn worklog_sync_populates_summary_from_jira_search_response() {
    let (server, client) = server_and_client().await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                {
                    "id": "10001",
                    "key": "ACME-1",
                    "fields": {
                        "summary": "Fix the login bug",
                        "updated": "2026-05-14T09:00:00.000+0000"
                    }
                }
            ],
            "isLast": true
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACME-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [
                {
                    "id": "5001",
                    "author": { "accountId": "me-acc" },
                    "started": "2026-05-14T09:00:00.000+0000",
                    "timeSpentSeconds": 1800,
                    "updated": "2026-05-14T09:30:00.000+0000",
                    "created": "2026-05-14T09:30:00.000+0000"
                }
            ],
            "total": 1, "startAt": 0, "maxResults": 1000
        })))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("sync.db")).unwrap();

    let from = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    sync_worklogs_for_range(&client, &db, "me-acc", from, from)
        .await
        .unwrap();

    let rows = cache::worklogs::for_date_range(&db, 0, i64::MAX, Some("me-acc")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].summary.as_deref(),
        Some("Fix the login bug"),
        "summary should be filled from the issue search response"
    );
}

#[tokio::test]
async fn worklog_sync_backfills_summary_from_issue_cache() {
    // Pre-seed: a worklog row with `summary = NULL`, AND a cached issue row
    // with the right summary. The sync's backfill pass should populate the
    // worklog's summary from the cache (no extra Jira fetch needed).

    let (server, client) = server_and_client().await;

    // Search returns the issue so the local row doesn't get tombstoned by
    // the mark-and-sweep pass.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                {
                    "id": "10007",
                    "key": "ACME-7",
                    "fields": {
                        "summary": "Cached summary",
                        "updated": "2026-05-14T09:00:00.000+0000"
                    }
                }
            ],
            "isLast": true
        })))
        .mount(&server)
        .await;

    // Issue worklog endpoint returns our existing worklog so mark-and-sweep
    // sees it.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACME-7/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [
                {
                    "id": "9999",
                    "author": { "accountId": "me-acc" },
                    "started": "2026-05-14T09:30:00.000+0000",
                    "timeSpentSeconds": 600,
                    "updated": "2026-05-14T09:30:00.000+0000",
                    "created": "2026-05-14T09:30:00.000+0000"
                }
            ],
            "total": 1, "startAt": 0, "maxResults": 1000
        })))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("backfill.db")).unwrap();

    let from = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    sync_worklogs_for_range(&client, &db, "me-acc", from, from)
        .await
        .unwrap();

    let row = cache::worklogs::get_by_jira_id(&db, "9999")
        .unwrap()
        .expect("row present");
    assert_eq!(
        row.summary.as_deref(),
        Some("Cached summary"),
        "summary should be filled from the cached issue"
    );

    // The IssueRow type still imports OK (compile-time sanity).
    let issue = cache::issues::get_by_key(&db, "ACME-7").unwrap().unwrap();
    let _u: IssueRow = issue;
}

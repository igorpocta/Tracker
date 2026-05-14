use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use tempfile::TempDir;
use tracker_lib::cache::Db;
use tracker_lib::jira::{
    adf::make_adf_comment,
    jql::{escape_quoted, DEFAULT_JQL},
    models::{map_issue_to_row, JiraIssue},
    sync_issues_from_jira, JiraClient, JiraError,
};
use wiremock::matchers::{basic_auth, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret-token";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client = JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string())
        .expect("client builds");
    (server, client)
}

// ---------- myself ----------

#[tokio::test]
async fn myself_returns_user() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "5b10ac8d82e05b22cc7d4ef5",
            "displayName": "Alice Example",
            "emailAddress": "alice@example.com"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let user = client.myself().await.expect("ok");
    assert_eq!(user.account_id, "5b10ac8d82e05b22cc7d4ef5");
    assert_eq!(user.display_name, "Alice Example");
    assert_eq!(user.email_address.as_deref(), Some("alice@example.com"));
}

#[tokio::test]
async fn myself_propagates_401_as_error() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;

    let err = client.myself().await.unwrap_err();
    assert!(matches!(err, JiraError::Unauthorized), "got {err:?}");
}

#[tokio::test]
async fn myself_propagates_other_api_errors() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = client.myself().await.unwrap_err();
    match err {
        JiraError::Api { status, body } => {
            assert_eq!(status, 500);
            assert!(body.contains("boom"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// ---------- search_jql ----------

#[tokio::test]
async fn search_jql_parses_first_page_with_next_token() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(body_partial_json(json!({
            "jql": DEFAULT_JQL,
            "fields": ["summary", "updated"],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                { "id": "10001", "key": "ABC-1", "fields": { "summary": "First" } },
                { "id": "10002", "key": "ABC-2", "fields": { "summary": "Second" } }
            ],
            "nextPageToken": "tok-2",
            "isLast": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = client
        .search_jql(DEFAULT_JQL, None, &["summary", "updated"])
        .await
        .expect("ok");
    assert_eq!(page.issues.len(), 2);
    assert_eq!(page.issues[0].key, "ABC-1");
    assert_eq!(page.next_page_token.as_deref(), Some("tok-2"));
    assert!(!page.is_last);
}

#[tokio::test]
async fn search_jql_handles_empty_page() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [],
            "isLast": true
        })))
        .mount(&server)
        .await;

    let page = client
        .search_jql(DEFAULT_JQL, None, &["summary"])
        .await
        .unwrap();
    assert!(page.issues.is_empty());
    assert!(page.is_last);
    assert!(page.next_page_token.is_none());
}

#[tokio::test]
async fn search_jql_forwards_page_token() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(json!({
            "nextPageToken": "tok-2",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [],
            "isLast": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    client
        .search_jql(DEFAULT_JQL, Some("tok-2"), &["summary"])
        .await
        .unwrap();
}

// ---------- add_worklog ----------

#[tokio::test]
async fn add_worklog_posts_started_and_timespent() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/ABC-1/worklog"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(body_partial_json(json!({
            "started": "2026-05-14T09:30:00.000+0000",
            "timeSpentSeconds": 1800,
            "comment": {
                "type": "doc",
                "version": 1
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "99999",
            "issueId": "10001",
            "timeSpentSeconds": 1800,
            "started": "2026-05-14T09:30:00.000+0000"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let resp = client
        .add_worklog("ABC-1", started, 1800, Some("Worked on it"))
        .await
        .expect("ok");

    assert_eq!(resp.id, "99999");
    assert_eq!(resp.time_spent_seconds, Some(1800));
}

#[tokio::test]
async fn add_worklog_without_comment_omits_field() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/XYZ-9/worklog"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 60
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "1",
            "timeSpentSeconds": 60
        })))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 0, 0, 0).unwrap();
    client
        .add_worklog("XYZ-9", started, 60, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn add_worklog_empty_comment_omits_field() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/XYZ-9/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"1"})))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 0, 0, 0).unwrap();
    // whitespace-only comment should still serialize without panicking
    client
        .add_worklog("XYZ-9", started, 60, Some("   "))
        .await
        .unwrap();
}

// ---------- ADF ----------

#[test]
fn make_adf_comment_produces_expected_shape() {
    let adf = make_adf_comment("Hello world").unwrap();
    let expected: Value = json!({
        "type": "doc",
        "version": 1,
        "content": [
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Hello world" }
                ]
            }
        ]
    });
    assert_eq!(adf, expected);
}

#[test]
fn make_adf_comment_empty_text_returns_none() {
    assert!(make_adf_comment("").is_none());
    assert!(make_adf_comment("   \n\t  ").is_none());
}

#[test]
fn make_adf_comment_trims_whitespace() {
    let adf = make_adf_comment("  hi  ").unwrap();
    assert_eq!(
        adf["content"][0]["content"][0]["text"].as_str(),
        Some("hi")
    );
}

// ---------- map_issue_to_row ----------

#[test]
fn map_issue_to_row_populates_core_fields() {
    let raw = r#"{
        "id": "10001",
        "key": "ABC-1",
        "fields": {
            "summary": "Fix login bug",
            "status": { "name": "In Progress", "statusCategory": { "key": "indeterminate", "name": "In Progress" } },
            "priority": { "name": "High", "id": "2" },
            "assignee": { "accountId": "abc123", "displayName": "Alice", "emailAddress": "alice@example.com" },
            "parent": { "key": "EPIC-1", "fields": { "summary": "Auth Epic", "issuetype": { "name": "Epic" } } },
            "issuetype": { "name": "Bug" },
            "timetracking": {
                "timeSpentSeconds": 3600,
                "originalEstimateSeconds": 7200,
                "remainingEstimateSeconds": 1800
            },
            "customfield_10014": "EPIC-1",
            "updated": "2026-05-14T09:30:00.000+0000"
        }
    }"#;
    let issue: JiraIssue = serde_json::from_str(raw).unwrap();
    let row = map_issue_to_row(&issue);

    assert_eq!(row.issue_key, "ABC-1");
    assert_eq!(row.issue_id.as_deref(), Some("10001"));
    assert_eq!(row.summary, "Fix login bug");
    assert_eq!(row.status_category.as_deref(), Some("indeterminate"));
    assert_eq!(row.priority_order, Some(2));
    assert_eq!(row.assignee_email.as_deref(), Some("alice@example.com"));
    assert_eq!(row.assignee_account_id.as_deref(), Some("abc123"));
    assert_eq!(row.parent_key.as_deref(), Some("EPIC-1"));
    assert_eq!(row.parent_summary.as_deref(), Some("Auth Epic"));
    assert_eq!(row.issue_type.as_deref(), Some("Bug"));
    assert_eq!(row.time_spent, Some(3600));
    assert_eq!(row.time_original_estimate, Some(7200));
    assert_eq!(row.time_estimate, Some(1800));
    assert_eq!(row.epic_key.as_deref(), Some("EPIC-1"));
    // 2026-05-14 09:30:00 UTC
    assert_eq!(row.updated_at, 1778751000);
}

#[test]
fn map_issue_to_row_handles_missing_optional_fields() {
    let raw = r#"{
        "id": "1",
        "key": "MIN-1",
        "fields": {
            "summary": "Bare bones",
            "updated": "2026-01-01T00:00:00.000+0000"
        }
    }"#;
    let issue: JiraIssue = serde_json::from_str(raw).unwrap();
    let row = map_issue_to_row(&issue);

    assert_eq!(row.issue_key, "MIN-1");
    assert_eq!(row.summary, "Bare bones");
    assert!(row.status_category.is_none());
    assert!(row.priority_order.is_none());
    assert!(row.assignee_email.is_none());
    assert!(row.assignee_account_id.is_none());
    assert!(row.parent_key.is_none());
    assert!(row.parent_summary.is_none());
    assert!(row.issue_type.is_none());
    assert!(row.time_spent.is_none());
    assert!(row.time_original_estimate.is_none());
    assert!(row.time_estimate.is_none());
    assert!(row.epic_key.is_none());
    assert_eq!(row.updated_at, 1767225600);
}

#[test]
fn map_issue_to_row_unparseable_updated_falls_back_to_zero() {
    let raw = r#"{ "id": "1", "key": "X-1", "fields": { "summary": "x", "updated": "garbage" } }"#;
    let issue: JiraIssue = serde_json::from_str(raw).unwrap();
    let row = map_issue_to_row(&issue);
    assert_eq!(row.updated_at, 0);
}

// ---------- JQL constant + helpers ----------

#[test]
fn default_jql_matches_plan() {
    assert_eq!(
        DEFAULT_JQL,
        r#"NOT (statusCategory = "Done" AND updated < "-14d") ORDER BY updated DESC"#
    );
}

#[test]
fn escape_quoted_escapes_quotes_and_backslashes() {
    assert_eq!(escape_quoted(r#"hello "world""#), r#"hello \"world\""#);
    assert_eq!(escape_quoted(r"a\b"), r"a\\b");
}

// ---------- full sync ----------

#[tokio::test]
async fn sync_issues_from_jira_walks_two_pages_into_sqlite() {
    let (server, client) = server_and_client().await;

    // Page 2: matched specifically by the presence of "nextPageToken" in the body.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(body_partial_json(json!({ "jql": DEFAULT_JQL })))
        .and(wiremock::matchers::body_string_contains("nextPageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                { "id": "20", "key": "P-20", "fields": { "summary": "Page2 first", "updated": "2026-05-01T00:00:00.000+0000" } }
            ],
            "isLast": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 1 (no token) — wiremock falls through to this generic matcher.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                { "id": "10", "key": "P-10", "fields": { "summary": "Page1 first", "updated": "2026-05-02T00:00:00.000+0000" } },
                { "id": "11", "key": "P-11", "fields": { "summary": "Page1 second", "updated": "2026-05-03T00:00:00.000+0000" } }
            ],
            "nextPageToken": "tok-2",
            "isLast": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("sync.db")).unwrap();

    let total = sync_issues_from_jira(&client, &db).await.expect("sync");
    assert_eq!(total, 3);

    let got = tracker_lib::cache::issues::get_by_key(&db, "P-10")
        .unwrap()
        .unwrap();
    assert_eq!(got.summary, "Page1 first");
    let got = tracker_lib::cache::issues::get_by_key(&db, "P-20")
        .unwrap()
        .unwrap();
    assert_eq!(got.summary, "Page2 first");
}

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use tempfile::TempDir;
use tracker_lib::cache::Db;
use tracker_lib::jira::{
    adf::{extract_adf_text, make_adf_comment},
    jql::{escape_quoted, DEFAULT_JQL},
    models::{map_issue_to_row, JiraIssue},
    sync_issues_from_jira, JiraClient, JiraError,
};
use wiremock::matchers::{basic_auth, body_partial_json, header, method, path, query_param};
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
        .search_jql(DEFAULT_JQL, None, &["summary", "updated"], 50)
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
        .search_jql(DEFAULT_JQL, None, &["summary"], 50)
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
        .search_jql(DEFAULT_JQL, Some("tok-2"), &["summary"], 50)
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

// ---------- update_worklog ----------

#[tokio::test]
async fn update_worklog_sends_only_provided_fields() {
    let (server, client) = server_and_client().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/99999"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(body_partial_json(json!({
            "started": "2026-05-14T09:30:00.000+0000",
            "timeSpentSeconds": 2400
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99999",
            "issueId": "10001",
            "timeSpentSeconds": 2400,
            "started": "2026-05-14T09:30:00.000+0000"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let resp = client
        .update_worklog("ABC-1", "99999", Some(started), Some(2400), None)
        .await
        .expect("ok");
    assert_eq!(resp.id, "99999");
    assert_eq!(resp.time_spent_seconds, Some(2400));
}

#[tokio::test]
async fn update_worklog_with_comment_includes_adf() {
    let (server, client) = server_and_client().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/99999"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 600,
            "comment": {
                "type": "doc",
                "version": 1
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "99999",
            "timeSpentSeconds": 600
        })))
        .expect(1)
        .mount(&server)
        .await;

    let resp = client
        .update_worklog("ABC-1", "99999", None, Some(600), Some("Updated comment"))
        .await
        .expect("ok");
    assert_eq!(resp.id, "99999");
}

#[tokio::test]
async fn update_worklog_404_returns_not_found() {
    let (server, client) = server_and_client().await;
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let err = client
        .update_worklog("ABC-1", "missing", None, Some(60), None)
        .await
        .unwrap_err();
    assert!(matches!(err, JiraError::WorklogNotFound), "got {err:?}");
}

// ---------- delete_worklog ----------

#[tokio::test]
async fn delete_worklog_returns_ok_on_204() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/99999"))
        .and(basic_auth(EMAIL, TOKEN))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .delete_worklog("ABC-1", "99999")
        .await
        .expect("ok");
}

#[tokio::test]
async fn delete_worklog_treats_404_as_not_found() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("gone"))
        .expect(1)
        .mount(&server)
        .await;

    let err = client
        .delete_worklog("ABC-1", "missing")
        .await
        .unwrap_err();
    assert!(matches!(err, JiraError::WorklogNotFound), "got {err:?}");
}

#[tokio::test]
async fn delete_worklog_500_returns_api_error() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/99999"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = client.delete_worklog("ABC-1", "99999").await.unwrap_err();
    match err {
        JiraError::Api { status, .. } => assert_eq!(status, 500),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn delete_worklog_401_returns_unauthorized() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/ABC-1/worklog/99999"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let err = client.delete_worklog("ABC-1", "99999").await.unwrap_err();
    assert!(matches!(err, JiraError::Unauthorized), "got {err:?}");
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
    // Phase 14: the sync no longer applies any restrictive WHERE clause; we
    // pull "everything visible, most-recently-updated first" and let the
    // pagination caps in `mod.rs` bound the total volume.
    assert_eq!(DEFAULT_JQL, r#"ORDER BY updated DESC"#);
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

// ---------- Phase 11A: ADF text extraction ----------

#[test]
fn extract_adf_text_handles_realistic_worklog_comment() {
    let comment = json!({
        "type": "doc",
        "version": 1,
        "content": [
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Fixed the " },
                    { "type": "text", "text": "login", "marks": [{ "type": "strong" }] },
                    { "type": "text", "text": " bug" }
                ]
            },
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Also added tests." }
                ]
            }
        ]
    });
    let text = extract_adf_text(&comment);
    // Should contain the words from both paragraphs.
    assert!(text.contains("login"));
    assert!(text.contains("bug"));
    assert!(text.contains("Also added tests."));
    // Paragraph break -> newline somewhere in the middle.
    assert!(text.contains('\n'));
}

// ---------- Phase 11A: worklog endpoints ----------

#[tokio::test]
async fn worklog_updated_since_parses_page() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/worklog/updated"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(query_param("since", "1700000000000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [
                { "worklogId": 1001, "updatedTime": 1700000001000_i64 },
                { "worklogId": 1002, "updatedTime": 1700000002000_i64 }
            ],
            "lastPage": false,
            "nextPage": "https://example.atlassian.net/rest/api/3/worklog/updated?since=1700000002000",
            "since": 1700000000000_i64,
            "until": 1700000002000_i64
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = client.worklog_updated_since(1_700_000_000_000).await.unwrap();
    assert_eq!(page.values.len(), 2);
    assert_eq!(page.values[0].worklog_id, 1001);
    assert_eq!(page.values[1].worklog_id, 1002);
    assert!(!page.last_page);
    assert!(page.next_page.is_some());
}

#[tokio::test]
async fn worklog_list_returns_details() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/worklog/list"))
        .and(basic_auth(EMAIL, TOKEN))
        .and(body_partial_json(json!({ "ids": [1001, 1002] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "1001",
                "issueId": "20001",
                "author": {
                    "accountId": "user-a",
                    "displayName": "Alice",
                    "emailAddress": "alice@example.com"
                },
                "started": "2026-05-14T09:30:00.000+0000",
                "timeSpentSeconds": 1800,
                "comment": {
                    "type": "doc",
                    "version": 1,
                    "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "Worked on it" }
                        ]}
                    ]
                },
                "updated": "2026-05-14T09:35:00.000+0000",
                "created": "2026-05-14T09:35:00.000+0000"
            },
            {
                "id": "1002",
                "issueId": "20002",
                "author": { "accountId": "user-b" },
                "started": "2026-05-14T10:00:00.000+0000",
                "timeSpentSeconds": 600,
                "updated": "2026-05-14T10:05:00.000+0000",
                "created": "2026-05-14T10:05:00.000+0000"
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let worklogs = client.worklog_list(&[1001, 1002]).await.unwrap();
    assert_eq!(worklogs.len(), 2);
    assert_eq!(worklogs[0].id, "1001");
    assert_eq!(worklogs[0].issue_id.as_deref(), Some("20001"));
    assert_eq!(worklogs[0].author.account_id, "user-a");
    assert_eq!(worklogs[0].time_spent_seconds, 1800);
    assert!(worklogs[0].comment.is_some());
    assert_eq!(worklogs[1].id, "1002");
    assert!(worklogs[1].comment.is_none());
}

#[tokio::test]
async fn issue_worklogs_paginates() {
    let (server, client) = server_and_client().await;

    // Page 2: startAt=2
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ABC-1/worklog"))
        .and(query_param("startAt", "2"))
        .and(query_param("maxResults", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [
                {
                    "id": "3",
                    "author": { "accountId": "user-a" },
                    "started": "2026-05-15T10:00:00.000+0000",
                    "timeSpentSeconds": 300,
                    "updated": "2026-05-15T10:05:00.000+0000",
                    "created": "2026-05-15T10:05:00.000+0000"
                }
            ],
            "total": 3,
            "startAt": 2,
            "maxResults": 2
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 1: startAt=0
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ABC-1/worklog"))
        .and(query_param("startAt", "0"))
        .and(query_param("maxResults", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [
                {
                    "id": "1",
                    "author": { "accountId": "user-a" },
                    "started": "2026-05-14T09:00:00.000+0000",
                    "timeSpentSeconds": 1800,
                    "updated": "2026-05-14T09:05:00.000+0000",
                    "created": "2026-05-14T09:05:00.000+0000"
                },
                {
                    "id": "2",
                    "author": { "accountId": "user-b" },
                    "started": "2026-05-14T10:00:00.000+0000",
                    "timeSpentSeconds": 600,
                    "updated": "2026-05-14T10:05:00.000+0000",
                    "created": "2026-05-14T10:05:00.000+0000"
                }
            ],
            "total": 3,
            "startAt": 0,
            "maxResults": 2
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page1 = client.issue_worklogs("ABC-1", 0, 2).await.unwrap();
    assert_eq!(page1.total, 3);
    assert_eq!(page1.worklogs.len(), 2);
    assert_eq!(page1.start_at, 0);
    assert_eq!(page1.max_results, 2);

    let page2 = client.issue_worklogs("ABC-1", 2, 2).await.unwrap();
    assert_eq!(page2.worklogs.len(), 1);
    assert_eq!(page2.worklogs[0].id, "3");
}

// ---------- Phase 11A: worklog sync ----------

#[tokio::test]
async fn sync_worklogs_for_range_filters_by_user_and_range() {
    use chrono::NaiveDate;
    use tracker_lib::jira::worklog_sync::sync_worklogs_for_range;

    let (server, client) = server_and_client().await;

    // JQL search returns one issue.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                {
                    "id": "10001",
                    "key": "ACME-1",
                    "fields": {
                        "summary": "Fix bug",
                        "updated": "2026-05-14T09:00:00.000+0000"
                    }
                }
            ],
            "isLast": true
        })))
        .mount(&server)
        .await;

    // Per-issue worklog endpoint returns three worklogs:
    //   - in range, by me -> KEEP
    //   - in range, by someone else -> FILTER OUT
    //   - by me but outside the range -> FILTER OUT
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/ACME-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [
                {
                    "id": "5001",
                    "author": { "accountId": "me-acc" },
                    "started": "2026-05-14T09:00:00.000+0000",
                    "timeSpentSeconds": 1800,
                    "comment": {
                        "type": "doc", "version": 1,
                        "content": [{ "type": "paragraph", "content": [
                            { "type": "text", "text": "real work" }
                        ]}]
                    },
                    "updated": "2026-05-14T09:30:00.000+0000",
                    "created": "2026-05-14T09:30:00.000+0000"
                },
                {
                    "id": "5002",
                    "author": { "accountId": "other-acc" },
                    "started": "2026-05-14T10:00:00.000+0000",
                    "timeSpentSeconds": 900,
                    "updated": "2026-05-14T10:30:00.000+0000",
                    "created": "2026-05-14T10:30:00.000+0000"
                },
                {
                    "id": "5003",
                    "author": { "accountId": "me-acc" },
                    "started": "2025-01-01T08:00:00.000+0000",
                    "timeSpentSeconds": 600,
                    "updated": "2025-01-01T08:30:00.000+0000",
                    "created": "2025-01-01T08:30:00.000+0000"
                }
            ],
            "total": 3,
            "startAt": 0,
            "maxResults": 1000
        })))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("sync.db")).unwrap();

    let from = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let count = sync_worklogs_for_range(&client, &db, "me-acc", from, to)
        .await
        .expect("sync ok");
    assert_eq!(count, 1, "exactly one entry should match");

    // Verify the surviving worklog landed in the DB with the right shape.
    let rows = tracker_lib::cache::worklogs::for_date_range(&db, 0, i64::MAX, Some("me-acc"))
        .unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.jira_worklog_id.as_deref(), Some("5001"));
    assert_eq!(row.duration_s, 1800);
    assert_eq!(row.source, "jira");
    assert_eq!(row.comment.as_deref(), Some("real work"));
}

// ---------- Phase 15: mark-and-sweep ----------

#[tokio::test]
async fn sync_marks_deleted_remote_worklogs_as_tombstoned() {
    use chrono::NaiveDate;
    use tracker_lib::cache::worklogs::{upsert_from_jira, WorklogRow};
    use tracker_lib::jira::worklog_sync::sync_worklogs_for_range;

    let (server, client) = server_and_client().await;

    // Pre-seed two existing rows for the same date range. Only "5001" comes
    // back from Jira this pass; "5002" should be tombstoned by the sweep.
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("sweep.db")).unwrap();

    let in_range_ts = 1778751000_i64; // 2026-05-14 09:30 UTC
    upsert_from_jira(
        &db,
        &WorklogRow {
            id: None,
            issue_key: "ACME-1".into(),
            issue_id: Some("10001".into()),
            summary: Some("S".into()),
            duration_s: 1800,
            started_at: in_range_ts,
            logged_at: in_range_ts,
            comment: None,
            jira_worklog_id: Some("5001".into()),
            author_account_id: Some("me-acc".into()),
            source: "jira".into(),
            updated_at_jira: Some(in_range_ts),
            pending_delete_at: None,
            tombstoned_at: None,
            pending_assignment: false,
        },
    )
    .unwrap();
    upsert_from_jira(
        &db,
        &WorklogRow {
            id: None,
            issue_key: "ACME-1".into(),
            issue_id: Some("10001".into()),
            summary: Some("S".into()),
            duration_s: 600,
            started_at: in_range_ts + 60,
            logged_at: in_range_ts + 60,
            comment: None,
            jira_worklog_id: Some("5002".into()),
            author_account_id: Some("me-acc".into()),
            source: "jira".into(),
            updated_at_jira: Some(in_range_ts + 60),
            pending_delete_at: None,
            tombstoned_at: None,
            pending_assignment: false,
        },
    )
    .unwrap();

    // Jira returns only 5001 now (5002 was deleted upstream).
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [
                { "id": "10001", "key": "ACME-1", "fields": { "summary": "S", "updated": "2026-05-14T09:00:00.000+0000" } }
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
                    "started": "2026-05-14T09:30:00.000+0000",
                    "timeSpentSeconds": 1800,
                    "updated": "2026-05-14T09:30:00.000+0000",
                    "created": "2026-05-14T09:30:00.000+0000"
                }
            ],
            "total": 1,
            "startAt": 0,
            "maxResults": 1000
        })))
        .mount(&server)
        .await;

    let from = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    sync_worklogs_for_range(&client, &db, "me-acc", from, to)
        .await
        .expect("ok");

    // Default query should now show only 5001 (5002 is tombstoned).
    let rows =
        tracker_lib::cache::worklogs::for_date_range(&db, 0, i64::MAX, Some("me-acc"))
            .unwrap();
    let ids: Vec<&str> = rows
        .iter()
        .map(|r| r.jira_worklog_id.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(ids, vec!["5001"]);

    // …but the diagnostic query includes the tombstoned row.
    let with_tomb =
        tracker_lib::cache::worklogs::for_date_range_including_tombstoned(
            &db,
            0,
            i64::MAX,
            Some("me-acc"),
        )
        .unwrap();
    assert_eq!(with_tomb.len(), 2);
    let tombstoned = with_tomb
        .iter()
        .find(|r| r.jira_worklog_id.as_deref() == Some("5002"))
        .expect("5002 row present");
    assert!(tombstoned.tombstoned_at.is_some());
}

#[test]
fn for_date_range_excludes_tombstoned() {
    use tracker_lib::cache::worklogs::{
        for_date_range, for_date_range_including_tombstoned, mark_tombstoned_by_jira_id,
        upsert_from_jira, WorklogRow,
    };

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("excl.db")).unwrap();

    let base = 1_700_000_000_i64;
    for jid in ["a", "b", "c"] {
        upsert_from_jira(
            &db,
            &WorklogRow {
                id: None,
                issue_key: "K-1".into(),
                duration_s: 600,
                started_at: base,
                logged_at: base,
                jira_worklog_id: Some(jid.into()),
                author_account_id: Some("me".into()),
                source: "jira".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    mark_tombstoned_by_jira_id(&db, "b", base + 60).unwrap();

    let default_rows = for_date_range(&db, 0, i64::MAX, Some("me")).unwrap();
    let default_ids: Vec<&str> = default_rows
        .iter()
        .map(|r| r.jira_worklog_id.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(default_ids.len(), 2);
    assert!(!default_ids.contains(&"b"));

    let all_rows =
        for_date_range_including_tombstoned(&db, 0, i64::MAX, Some("me")).unwrap();
    assert_eq!(all_rows.len(), 3);
}

#[test]
fn purge_old_tombstoned_hard_deletes_old_rows() {
    use tracker_lib::cache::worklogs::{
        count, for_date_range_including_tombstoned, mark_tombstoned_by_jira_id,
        purge_old_tombstoned, upsert_from_jira, WorklogRow,
    };

    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("purge.db")).unwrap();

    let base = 1_700_000_000_i64;
    for jid in ["old", "new"] {
        upsert_from_jira(
            &db,
            &WorklogRow {
                id: None,
                issue_key: "K-1".into(),
                duration_s: 600,
                started_at: base,
                logged_at: base,
                jira_worklog_id: Some(jid.into()),
                author_account_id: Some("me".into()),
                source: "jira".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    mark_tombstoned_by_jira_id(&db, "old", 100).unwrap();
    mark_tombstoned_by_jira_id(&db, "new", 10_000).unwrap();

    let removed = purge_old_tombstoned(&db, 1_000).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(count(&db).unwrap(), 1);

    // The remaining row should be "new" (still tombstoned but within window).
    let rows = for_date_range_including_tombstoned(&db, 0, i64::MAX, Some("me")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].jira_worklog_id.as_deref(), Some("new"));
}

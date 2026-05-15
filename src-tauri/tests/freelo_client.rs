//! Wiremock coverage for the Freelo API client.
//!
//! These tests do NOT touch the real Freelo API — they stand up a local
//! HTTP server with `wiremock`, point the [`FreeloClient`] at it, and assert
//! both the outgoing request shapes and the parsing of canned responses.

use chrono::NaiveDate;
use serde_json::{json, Value};
use tracker_lib::freelo::{FreeloClient, FreeloError};
use wiremock::matchers::{basic_auth, body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const KEY: &str = "freelo-api-key";

async fn server_and_client() -> (MockServer, FreeloClient) {
    let server = MockServer::start().await;
    let client = FreeloClient::new(server.uri(), EMAIL.into(), KEY.into()).unwrap();
    (server, client)
}

// ---------- me / users-manage-workers ----------

#[tokio::test]
async fn me_returns_authenticated_user() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/users/manage-workers"))
        .and(basic_auth(EMAIL, KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": 7,
                "email": "alice@example.com",
                "first_name": "Alice",
                "last_name": "Example"
            },
            {
                "id": 99,
                "email": "bob@example.com",
                "first_name": "Bob",
                "last_name": "Builder"
            }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let user = client.me().await.expect("ok");
    assert_eq!(user.id, 7);
    assert_eq!(user.best_name(), "Alice Example");
}

#[tokio::test]
async fn me_propagates_401_as_unauthorized() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/users/manage-workers"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;

    let err = client.me().await.unwrap_err();
    assert!(matches!(err, FreeloError::Unauthorized), "got {err:?}");
}

// ---------- list_projects ----------

#[tokio::test]
async fn list_projects_parses_response() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .and(basic_auth(EMAIL, KEY))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1, "name": "Marketing", "state": "active" },
            { "id": 2, "name": "Sales", "state": "finished" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok");
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].id, 1);
    assert_eq!(projects[0].name, "Marketing");
    assert_eq!(projects[1].state, "finished");
}

#[tokio::test]
async fn list_projects_handles_wrapped_response() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "projects": [
                { "id": 5, "name": "Web" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Web");
    // default state when not provided.
    assert_eq!(projects[0].state, "active");
}

// ---------- list_tasks_for_project ----------

#[tokio::test]
async fn list_tasks_flattens_tasklist_response() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/project/10/all-tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasklists": [
                {
                    "id": 100,
                    "tasks": [
                        { "id": 1, "name": "First", "state": "active" },
                        { "id": 2, "name": "Second", "state": "active" }
                    ]
                },
                {
                    "id": 101,
                    "tasks": [
                        { "id": 3, "name": "Third", "state": "finished" }
                    ]
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let tasks = client.list_tasks_for_project(10).await.expect("ok");
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].id, 1);
    assert_eq!(tasks[0].tasklist_id, Some(100));
    // project_id back-filled from the URL when missing.
    assert_eq!(tasks[0].project_id, Some(10));
    assert_eq!(tasks[2].state, "finished");
}

// ---------- create_work_report ----------

#[tokio::test]
async fn create_work_report_posts_correct_body() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .and(basic_auth(EMAIL, KEY))
        .and(body_partial_json(json!({
            "minutes": 30,
            "date_reported": "2026-05-14",
            "description": "did the thing"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 555,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2026-05-14",
            "description": "did the thing",
            "user_id": 7
        })))
        .expect(1)
        .mount(&server)
        .await;

    let date = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let resp = client
        .create_work_report(42, date, 30, Some("did the thing"))
        .await
        .expect("ok");
    assert_eq!(resp.id, 555);
    assert_eq!(resp.task_id, 42);
    assert_eq!(resp.minutes, 30);
    assert_eq!(resp.date_reported, "2026-05-14");
    assert_eq!(resp.user_id, 7);
}

#[tokio::test]
async fn create_work_report_unwraps_wrapped_response() {
    let (server, client) = server_and_client().await;
    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "work_report": {
                "id": 9,
                "task_id": 42,
                "minutes": 5,
                "date_reported": "2026-05-14",
                "user_id": 7
            }
        })))
        .mount(&server)
        .await;

    let date = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let resp = client.create_work_report(42, date, 5, None).await.expect("ok");
    assert_eq!(resp.id, 9);
}

// ---------- delete_work_report ----------

#[tokio::test]
async fn delete_work_report_returns_ok_on_204() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/work-reports/77"))
        .and(basic_auth(EMAIL, KEY))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.delete_work_report(77).await.expect("ok");
}

#[tokio::test]
async fn delete_work_report_404_returns_not_found() {
    let (server, client) = server_and_client().await;
    Mock::given(method("DELETE"))
        .and(path("/work-reports/77"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = client.delete_work_report(77).await.unwrap_err();
    assert!(
        matches!(err, FreeloError::WorkReportNotFound),
        "got {err:?}"
    );
}

// ---------- 429 retry ----------

#[tokio::test]
async fn rate_limit_retry_uses_retry_after() {
    let (server, client) = server_and_client().await;
    // First call: 429 with Retry-After: 0. Second: 200.
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string("slow down"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1, "name": "Marketing", "state": "active" }
        ])))
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok after retry");
    assert_eq!(projects.len(), 1);
}

// ---------- list_work_reports ----------

#[tokio::test]
async fn list_work_reports_filters_to_user() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/timesheets"))
        .and(query_param("date_from", "2026-05-01"))
        .and(query_param("date_to", "2026-05-14"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1, "task_id": 10, "minutes": 30, "date_reported": "2026-05-14", "user_id": 7 },
            { "id": 2, "task_id": 11, "minutes": 45, "date_reported": "2026-05-13", "user_id": 99 }
        ])))
        .mount(&server)
        .await;

    let from = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let entries = client.list_work_reports(from, to, 7).await.expect("ok");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].user_id, 7);
}

#[tokio::test]
async fn list_work_reports_flattens_nested_task_and_user() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/timesheets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": 5,
                "task": { "id": 100 },
                "worker": { "id": 7 },
                "minutes": 12,
                "date_reported": "2026-05-14"
            }
        ])))
        .mount(&server)
        .await;

    let from = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let entries = client.list_work_reports(from, to, 7).await.expect("ok");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].task_id, 100);
    assert_eq!(entries[0].user_id, 7);
    assert_eq!(entries[0].minutes, 12);
}

// Sanity check: dropping the `_` (mark variable as used) by checking the
// projects helper produces a deterministic output.
#[tokio::test]
async fn projects_keys_are_freelo_synthetic() {
    use tracker_lib::freelo::{project_key, task_key};
    assert_eq!(project_key(1), "FRL-P-1");
    assert_eq!(task_key(2), "FRL-2");
    // Suppress unused-import warnings.
    let _ = json!({});
    let _: Value = serde_json::json!({});
}

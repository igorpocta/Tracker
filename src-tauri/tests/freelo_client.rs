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

// ---------- me / /users/me health check ----------

#[tokio::test]
async fn me_returns_synthetic_user_on_success() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .and(basic_auth(EMAIL, KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": "success"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let user = client.me().await.expect("ok");
    // /users/me carries no user details, so client synthesizes id=0 + email.
    assert_eq!(user.id, 0);
    assert_eq!(user.email.as_deref(), Some(EMAIL));
    assert!(user.best_name().contains("Alice"));
}

#[tokio::test]
async fn me_propagates_401_as_unauthorized() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
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

#[tokio::test]
async fn list_projects_parses_real_freelo_v1_state_object() {
    // Real Freelo v1 response shape (verified against the live API):
    //   "state": { "id": 1, "state": "active" }   ← state is an OBJECT
    // Previously our struct expected a string and parsing silently dropped
    // every project. This test guards against that regression.
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 2, "count": 2, "page": 0, "per_page": 100,
            "data": { "projects": [
                {
                    "id": 57447,
                    "name": "SAB vývoj - Ostatní",
                    "owner": { "id": 46694, "fullname": "Pavel Černý" },
                    "state": { "id": 1, "state": "active" },
                    "date_add": "2019-07-10 16:38:48"
                },
                {
                    "id": 305791,
                    "name": "SAB vývoj - II.",
                    "owner": { "id": 46694, "fullname": "Pavel Černý" },
                    "state": { "id": 2, "state": "archived" },
                    "date_add": "2023-10-04 12:54:54"
                }
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok");
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].id, 57447);
    assert_eq!(projects[0].state, "active");
    assert_eq!(projects[1].state, "archived");
}

#[tokio::test]
async fn list_projects_walks_through_multiple_pages() {
    // When the server reports total > out.len() we keep paginating.
    let (server, client) = server_and_client().await;
    // Page 0
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .and(wiremock::matchers::query_param("p", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 4, "count": 2, "page": 0, "per_page": 2,
            "data": { "projects": [
                { "id": 1, "name": "A", "state": "active" },
                { "id": 2, "name": "B", "state": "active" }
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;
    // Page 1
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .and(wiremock::matchers::query_param("p", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 4, "count": 2, "page": 1, "per_page": 2,
            "data": { "projects": [
                { "id": 3, "name": "C", "state": "active" },
                { "id": 4, "name": "D", "state": "finished" }
            ]}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok");
    let ids: Vec<i64> = projects.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn list_projects_reads_canonical_paginated_shape() {
    // Real Freelo v1 shape per https://api.freelo.io/docs/v1/freelo-api:
    //   { total, count, page, per_page, data: { projects: [...] } }
    // /all-projects returns BOTH owned and invited mixed into data.projects.
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/all-projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 3,
            "count": 3,
            "page": 1,
            "per_page": 50,
            "data": {
                "projects": [
                    { "id": 1, "name": "My own", "state": "active" },
                    { "id": 2, "name": "Klient X", "state": "active" },
                    { "id": 42, "name": "SAB · Klient", "state": "active" }
                ]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let projects = client.list_projects().await.expect("ok");
    let ids: Vec<i64> = projects.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![1, 2, 42]);
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
            "note": "did the thing"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 555,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2026-05-14",
            "note": "did the thing",
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
    let resp = client
        .create_work_report(42, date, 5, None)
        .await
        .expect("ok");
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
async fn list_work_reports_uses_real_freelo_v1_shape() {
    // Verified against the live API — /work-reports returns:
    //   { total, count, page, per_page, data: { reports: [...] } }
    // Each report nests task/project/author/worker objects + `note` field
    // (we flatten to task_id/user_id/description on parse).
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/work-reports"))
        .and(query_param("date_reported_range[date_from]", "2026-04-01"))
        .and(query_param("date_reported_range[date_to]", "2026-04-15"))
        .and(query_param("users_ids[]", "140342"))
        .and(query_param("projects_ids[]", "446399"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 1, "count": 1, "page": 0, "per_page": 100,
            "data": {
                "reports": [
                    {
                        "id": 10037133,
                        "date_add": "2026-04-15T13:13:46+02:00",
                        "date_reported": "2026-04-15T12:24:27+02:00",
                        "note": "Optimalizace",
                        "minutes": 50,
                        "task": { "id": 27660453, "name": "Opt fáze 2" },
                        "project": { "id": 446399, "name": "SAB" },
                        "author": { "id": 140342, "fullname": "Igor" },
                        "worker": { "id": 140342, "fullname": "Igor" },
                        "date_edited_at": "2026-04-15T13:13:46+02:00"
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let from = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
    let entries = client
        .list_work_reports(from, to, 140342, &[446399])
        .await
        .expect("ok");
    assert_eq!(entries.len(), 1);
    let r = &entries[0];
    assert_eq!(r.id, 10037133);
    assert_eq!(r.task_id, 27660453);
    assert_eq!(r.user_id, 140342);
    assert_eq!(r.minutes, 50);
    assert_eq!(r.description.as_deref(), Some("Optimalizace"));
}

#[tokio::test]
async fn list_work_reports_filters_other_users_client_side() {
    let (server, client) = server_and_client().await;
    Mock::given(method("GET"))
        .and(path("/work-reports"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total": 2, "count": 2, "page": 0, "per_page": 100,
            "data": {
                "reports": [
                    { "id": 1, "minutes": 30, "date_reported": "2026-05-14",
                      "task": {"id": 10}, "author": {"id": 7} },
                    { "id": 2, "minutes": 45, "date_reported": "2026-05-13",
                      "task": {"id": 11}, "author": {"id": 99} }
                ]
            }
        })))
        .mount(&server)
        .await;

    let from = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
    let entries = client
        .list_work_reports(from, to, 7, &[])
        .await
        .expect("ok");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].user_id, 7);
}

// Sanity check: synthetic key helpers produce the user-visible prefix.
#[tokio::test]
async fn projects_keys_are_freelo_synthetic() {
    use tracker_lib::freelo::{project_key, task_key};
    assert_eq!(project_key(1), "FREELO-P-1");
    assert_eq!(task_key(2), "FREELO-2");
    let _ = json!({});
    let _: Value = serde_json::json!({});
}

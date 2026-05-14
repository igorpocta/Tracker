//! Tests for Phase 18A — Item 14: 429 rate-limit handling.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use serde_json::json;
use tracker_lib::jira::{JiraClient, JiraError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client =
        JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string()).expect("ok");
    (server, client)
}

/// Custom Respond impl: first N calls return 429 with given Retry-After,
/// subsequent calls return 200 with the supplied body.
struct FlakyRespond {
    fail_count: u32,
    retry_after: Option<String>,
    success_body: serde_json::Value,
    seen: Arc<AtomicU32>,
}

impl Respond for FlakyRespond {
    fn respond(&self, _req: &wiremock::Request) -> ResponseTemplate {
        let n = self.seen.fetch_add(1, Ordering::SeqCst);
        if n < self.fail_count {
            let mut tpl = ResponseTemplate::new(429);
            if let Some(ra) = &self.retry_after {
                tpl = tpl.insert_header("Retry-After", ra.as_str());
            }
            tpl
        } else {
            ResponseTemplate::new(200).set_body_json(self.success_body.clone())
        }
    }
}

#[tokio::test]
async fn respects_retry_after_seconds_header() {
    let (server, client) = server_and_client().await;
    let counter = Arc::new(AtomicU32::new(0));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(FlakyRespond {
            fail_count: 1,
            retry_after: Some("1".into()),
            success_body: json!({
                "accountId": "abc",
                "displayName": "Alice"
            }),
            seen: counter.clone(),
        })
        .mount(&server)
        .await;

    let user = client.myself().await.expect("retried OK");
    assert_eq!(user.account_id, "abc");
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn exponential_backoff_when_no_retry_after_header() {
    let (server, client) = server_and_client().await;
    let counter = Arc::new(AtomicU32::new(0));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(FlakyRespond {
            fail_count: 2, // 429, 429, then 200
            retry_after: None,
            success_body: json!({ "accountId": "abc", "displayName": "Alice" }),
            seen: counter.clone(),
        })
        .mount(&server)
        .await;

    let started = std::time::Instant::now();
    let user = client.myself().await.expect("retried OK");
    assert_eq!(user.account_id, "abc");
    // Two backoffs: 2^0 = 1s and 2^1 = 2s; allow a generous tolerance.
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(2),
        "expected at least 2s of total wait; got {:?}",
        started.elapsed()
    );
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn gives_up_after_max_retries() {
    let (server, client) = server_and_client().await;
    let counter = Arc::new(AtomicU32::new(0));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(FlakyRespond {
            // 100 failures so MAX_RETRIES (=3) is exceeded.
            fail_count: 100,
            retry_after: Some("1".into()),
            success_body: json!({}),
            seen: counter.clone(),
        })
        .mount(&server)
        .await;

    let err = client.myself().await.expect_err("should fail");
    assert!(matches!(err, JiraError::RateLimited { .. }), "got {err:?}");
    // We attempt: initial + 3 retries = 4 total
    assert_eq!(counter.load(Ordering::SeqCst), 4);
}

#[test]
fn retry_after_header_parser_accepts_seconds() {
    use reqwest::header::HeaderValue;
    let v = HeaderValue::from_static("30");
    let parsed = tracker_lib::jira::client::parse_retry_after_header(Some(&v));
    assert_eq!(parsed, Some(30));
}

#[test]
fn retry_after_header_parser_rejects_garbage() {
    use reqwest::header::HeaderValue;
    let v = HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT");
    let parsed = tracker_lib::jira::client::parse_retry_after_header(Some(&v));
    assert_eq!(parsed, None);
}

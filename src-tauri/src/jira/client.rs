use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::json;
use thiserror::Error;
use url::Url;

use super::adf::make_adf_comment;
use super::models::{
    IssueWorklogsPage, JiraIssue, JiraUser, JiraWorklog, SearchPage, WorklogResponse,
    WorklogUpdatedPage,
};

/// Maximum number of retries for a single Jira request after a 429 response.
pub const MAX_RETRIES: u32 = 3;
/// Cap on the wait-before-retry derived from `Retry-After` or backoff. Prevents
/// the app from blocking indefinitely if Jira sends back an absurd value.
pub const MAX_RETRY_WAIT_SECS: u64 = 60;

/// Errors produced by the Jira API client.
#[derive(Debug, Error)]
pub enum JiraError {
    #[error("invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("unauthorized")]
    Unauthorized,
    #[error("api error: status={status} body={body}")]
    Api { status: u16, body: String },
    #[error("chrono parse: {0}")]
    Chrono(#[from] chrono::ParseError),
    /// Returned by [`JiraClient::delete_worklog`] (and update) when Jira
    /// responds with a 404. Callers may want to treat this as "already gone,
    /// OK" rather than a hard failure.
    #[error("worklog not found")]
    WorklogNotFound,
    /// Returned (internally) when Jira responds with HTTP 429 Too Many
    /// Requests. `retry_after_secs` is parsed from the `Retry-After` header
    /// when present (seconds form only — we do not parse HTTP-date because
    /// Jira always emits seconds in practice).
    ///
    /// Callers should not observe this variant directly: the [`with_retry`]
    /// wrapper transparently retries up to [`MAX_RETRIES`] times. Only if all
    /// attempts are exhausted does this bubble out (currently as
    /// `JiraError::Api { status: 429 }` after the wrapper converts).
    #[error("rate limited (retry after {retry_after_secs:?} seconds)")]
    RateLimited { retry_after_secs: Option<u64> },
}

/// Typed Jira Cloud API client with Basic auth.
#[derive(Debug, Clone)]
pub struct JiraClient {
    http: Client,
    base_url: Url,
    email: String,
    token: String,
}

/// Sleep wrapper kept behind a function pointer so unit tests can stub it out
/// (no real wall-clock waiting in tests).
async fn default_sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// Generic retry-with-backoff for Jira API calls.
///
/// On [`JiraError::RateLimited`]:
/// - If `retry_after_secs` is `Some(n)`, wait `n` seconds.
/// - Otherwise wait `2^attempt` seconds (1, 2, 4 for attempts 0..3).
/// - Cap any computed wait at [`MAX_RETRY_WAIT_SECS`].
/// - Give up after [`MAX_RETRIES`] attempts.
///
/// All other errors are returned immediately without retrying.
pub async fn with_retry<F, Fut, T>(f: F) -> Result<T, JiraError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, JiraError>>,
{
    with_retry_using(f, default_sleep).await
}

/// Test-friendly variant of [`with_retry`] that accepts a custom sleep
/// function. Production code calls [`with_retry`].
pub(crate) async fn with_retry_using<F, Fut, T, S, SFut>(mut f: F, sleep: S) -> Result<T, JiraError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, JiraError>>,
    S: Fn(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut attempt: u32 = 0;
    loop {
        match f().await {
            Err(JiraError::RateLimited { retry_after_secs }) if attempt < MAX_RETRIES => {
                let wait = retry_after_secs
                    .unwrap_or_else(|| 2u64.saturating_pow(attempt))
                    .min(MAX_RETRY_WAIT_SECS);
                sleep(Duration::from_secs(wait)).await;
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Parse a `Retry-After` header value. Returns `None` for malformed input or
/// HTTP-date forms (we only support seconds because Jira always emits that).
pub fn parse_retry_after_header(h: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let v = h?;
    let s = v.to_str().ok()?.trim();
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    None
}

impl JiraClient {
    /// Build a new client.
    ///
    /// `base_url` should look like `https://example.atlassian.net` (no trailing `/rest/...`).
    pub fn new(base_url: String, email: String, token: String) -> Result<Self, JiraError> {
        let base_url = Url::parse(&base_url)?;
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("Tracker/0.1"));

        let http = Client::builder()
            .default_headers(headers)
            .use_rustls_tls()
            .gzip(true)
            .build()?;

        Ok(Self {
            http,
            base_url,
            email,
            token,
        })
    }

    fn url(&self, path: &str) -> Result<Url, JiraError> {
        // join() on a Url replaces the entire path if the input starts with `/`,
        // which is what we want for `/rest/api/3/...`.
        Ok(self.base_url.join(path)?)
    }

    /// The base URL the client was configured with (e.g.
    /// `https://example.atlassian.net/`). Used by link-builder helpers that
    /// need to construct `<base>/browse/KEY` style URLs without re-parsing.
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, JiraError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(JiraError::Unauthorized);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after_secs = parse_retry_after_header(resp.headers().get("Retry-After"));
            return Err(JiraError::RateLimited { retry_after_secs });
        }
        let code = status.as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(JiraError::Api { status: code, body })
    }

    /// `GET /rest/api/3/myself` — current user.
    pub async fn myself(&self) -> Result<JiraUser, JiraError> {
        let url = self.url("/rest/api/3/myself")?;
        with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<JiraUser>().await?)
        })
        .await
    }

    /// `POST /rest/api/3/search/jql` — JQL search with pagination (new endpoint shape).
    ///
    /// `page_token` is the opaque `nextPageToken` returned by Jira on the previous
    /// page. Pass `None` for the first page. `max_results` controls the
    /// per-page page size; Jira caps it server-side (typically at 100).
    pub async fn search_jql(
        &self,
        jql: &str,
        page_token: Option<&str>,
        fields: &[&str],
        max_results: u32,
    ) -> Result<SearchPage, JiraError> {
        let url = self.url("/rest/api/3/search/jql")?;
        let mut body = json!({
            "jql": jql,
            "fields": fields,
            "maxResults": max_results,
        });
        if let Some(tok) = page_token {
            body["nextPageToken"] = json!(tok);
        }

        with_retry(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .json(&body)
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<SearchPage>().await?)
        })
        .await
    }

    /// `POST /rest/api/3/issue/{key}/worklog` — add a worklog with an optional ADF comment.
    ///
    /// `started` is formatted as `YYYY-MM-DDTHH:MM:SS.SSS+0000` (Jira's required shape).
    /// `comment` is converted via [`make_adf_comment`]; empty/whitespace strings are
    /// dropped so the field is omitted from the request body entirely.
    pub async fn add_worklog(
        &self,
        issue_key: &str,
        started: DateTime<Utc>,
        time_spent_seconds: i64,
        comment: Option<&str>,
    ) -> Result<WorklogResponse, JiraError> {
        let url = self.url(&format!("/rest/api/3/issue/{issue_key}/worklog"))?;
        let started_str = started.format("%Y-%m-%dT%H:%M:%S%.3f+0000").to_string();

        let mut body = json!({
            "started": started_str,
            "timeSpentSeconds": time_spent_seconds,
        });
        if let Some(text) = comment {
            if let Some(adf) = make_adf_comment(text) {
                body["comment"] = adf;
            }
        }

        with_retry(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .json(&body)
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<WorklogResponse>().await?)
        })
        .await
    }

    /// `PUT /rest/api/3/issue/{key}/worklog/{id}` — partially update a worklog.
    ///
    /// Builds the body with only the fields that are `Some(_)`. `started` uses
    /// the same `%Y-%m-%dT%H:%M:%S%.3f+0000` format as [`Self::add_worklog`].
    /// `comment` is converted to ADF via [`make_adf_comment`]; empty / `None`
    /// comments are omitted from the request body entirely.
    ///
    /// Returns the updated `WorklogResponse`. A 404 from Jira is surfaced as
    /// [`JiraError::WorklogNotFound`] so callers can react to the
    /// "worklog already gone" case.
    pub async fn update_worklog(
        &self,
        issue_key: &str,
        worklog_id: &str,
        started: Option<DateTime<Utc>>,
        time_spent_seconds: Option<i64>,
        comment: Option<&str>,
    ) -> Result<WorklogResponse, JiraError> {
        let url = self.url(&format!(
            "/rest/api/3/issue/{issue_key}/worklog/{worklog_id}"
        ))?;

        let mut body = serde_json::Map::new();
        if let Some(started) = started {
            body.insert(
                "started".to_string(),
                serde_json::Value::String(started.format("%Y-%m-%dT%H:%M:%S%.3f+0000").to_string()),
            );
        }
        if let Some(secs) = time_spent_seconds {
            body.insert(
                "timeSpentSeconds".to_string(),
                serde_json::Value::Number(secs.into()),
            );
        }
        if let Some(text) = comment {
            if let Some(adf) = make_adf_comment(text) {
                body.insert("comment".to_string(), adf);
            }
        }

        with_retry(|| async {
            let resp = self
                .http
                .put(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .json(&serde_json::Value::Object(body.clone()))
                .send()
                .await?;

            let status = resp.status();
            if status == StatusCode::NOT_FOUND {
                return Err(JiraError::WorklogNotFound);
            }
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<WorklogResponse>().await?)
        })
        .await
    }

    /// `DELETE /rest/api/3/issue/{key}/worklog/{id}` — remove a worklog.
    ///
    /// Returns `Ok(())` on 204 No Content. A 404 is surfaced as
    /// [`JiraError::WorklogNotFound`] (the row is already gone — callers
    /// often treat this as success and just tombstone the local row).
    pub async fn delete_worklog(&self, issue_key: &str, worklog_id: &str) -> Result<(), JiraError> {
        let url = self.url(&format!(
            "/rest/api/3/issue/{issue_key}/worklog/{worklog_id}"
        ))?;
        with_retry(|| async {
            let resp = self
                .http
                .delete(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .send()
                .await?;

            let status = resp.status();
            if status == StatusCode::NO_CONTENT || status.is_success() {
                return Ok(());
            }
            if status == StatusCode::NOT_FOUND {
                return Err(JiraError::WorklogNotFound);
            }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(JiraError::Unauthorized);
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after_secs = parse_retry_after_header(resp.headers().get("Retry-After"));
                return Err(JiraError::RateLimited { retry_after_secs });
            }
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(JiraError::Api { status: code, body })
        })
        .await
    }

    /// `GET /rest/api/3/worklog/updated?since={ms_epoch}` — list worklog ids
    /// that have been created or modified since `since_ms` (Unix epoch in
    /// milliseconds).
    ///
    /// Returns one page of `{ worklogId, updatedTime }`; the caller is
    /// responsible for following `nextPage` if `lastPage == false`.
    pub async fn worklog_updated_since(
        &self,
        since_ms: i64,
    ) -> Result<WorklogUpdatedPage, JiraError> {
        let mut url = self.url("/rest/api/3/worklog/updated")?;
        url.query_pairs_mut()
            .append_pair("since", &since_ms.to_string());

        with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<WorklogUpdatedPage>().await?)
        })
        .await
    }

    /// `POST /rest/api/3/worklog/list` — fetch full worklog details for the
    /// given ids. Jira's documented per-call max is 1000; callers should batch.
    pub async fn worklog_list(&self, ids: &[i64]) -> Result<Vec<JiraWorklog>, JiraError> {
        let url = self.url("/rest/api/3/worklog/list")?;
        let body = json!({ "ids": ids });

        with_retry(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .json(&body)
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<Vec<JiraWorklog>>().await?)
        })
        .await
    }

    /// `GET /rest/api/3/issue/{key}/worklog?startAt=N&maxResults=M` —
    /// paginate over the full worklog list for a single issue.
    ///
    /// Note: the worklogs returned by this endpoint do **not** populate the
    /// `issueId` field on each entry (it's known from the URL). Callers that
    /// need `issueId` should set it from the corresponding issue.
    pub async fn issue_worklogs(
        &self,
        issue_key: &str,
        start_at: u32,
        max_results: u32,
    ) -> Result<IssueWorklogsPage, JiraError> {
        let mut url = self.url(&format!("/rest/api/3/issue/{issue_key}/worklog"))?;
        url.query_pairs_mut()
            .append_pair("startAt", &start_at.to_string())
            .append_pair("maxResults", &max_results.to_string());

        with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<IssueWorklogsPage>().await?)
        })
        .await
    }

    /// `GET /rest/api/3/issue/{key}?fields=summary,issuetype` — fetch a single
    /// issue (Phase 18A: used by worklog_sync to populate missing summaries).
    pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue, JiraError> {
        let mut url = self.url(&format!("/rest/api/3/issue/{issue_key}"))?;
        url.query_pairs_mut()
            .append_pair("fields", "summary,issuetype,parent,status,updated");

        with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.token))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<JiraIssue>().await?)
        })
        .await
    }
}

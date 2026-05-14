use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::json;
use thiserror::Error;
use url::Url;

use super::adf::make_adf_comment;
use super::models::{
    IssueWorklogsPage, JiraUser, JiraWorklog, SearchPage, WorklogResponse, WorklogUpdatedPage,
};

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
}

/// Typed Jira Cloud API client with Basic auth.
#[derive(Debug, Clone)]
pub struct JiraClient {
    http: Client,
    base_url: Url,
    email: String,
    token: String,
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

    async fn check_status(
        resp: reqwest::Response,
    ) -> Result<reqwest::Response, JiraError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(JiraError::Unauthorized);
        }
        let code = status.as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(JiraError::Api { status: code, body })
    }

    /// `GET /rest/api/3/myself` — current user.
    pub async fn myself(&self) -> Result<JiraUser, JiraError> {
        let url = self.url("/rest/api/3/myself")?;
        let resp = self
            .http
            .get(url)
            .basic_auth(&self.email, Some(&self.token))
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<JiraUser>().await?)
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

        let resp = self
            .http
            .post(url)
            .basic_auth(&self.email, Some(&self.token))
            .json(&body)
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<SearchPage>().await?)
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

        let resp = self
            .http
            .post(url)
            .basic_auth(&self.email, Some(&self.token))
            .json(&body)
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<WorklogResponse>().await?)
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
                serde_json::Value::String(
                    started.format("%Y-%m-%dT%H:%M:%S%.3f+0000").to_string(),
                ),
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

        let resp = self
            .http
            .put(url)
            .basic_auth(&self.email, Some(&self.token))
            .json(&serde_json::Value::Object(body))
            .send()
            .await?;

        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Err(JiraError::WorklogNotFound);
        }
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<WorklogResponse>().await?)
    }

    /// `DELETE /rest/api/3/issue/{key}/worklog/{id}` — remove a worklog.
    ///
    /// Returns `Ok(())` on 204 No Content. A 404 is surfaced as
    /// [`JiraError::WorklogNotFound`] (the row is already gone — callers
    /// often treat this as success and just tombstone the local row).
    pub async fn delete_worklog(
        &self,
        issue_key: &str,
        worklog_id: &str,
    ) -> Result<(), JiraError> {
        let url = self.url(&format!(
            "/rest/api/3/issue/{issue_key}/worklog/{worklog_id}"
        ))?;
        let resp = self
            .http
            .delete(url)
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
        let code = status.as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(JiraError::Api { status: code, body })
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

        let resp = self
            .http
            .get(url)
            .basic_auth(&self.email, Some(&self.token))
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<WorklogUpdatedPage>().await?)
    }

    /// `POST /rest/api/3/worklog/list` — fetch full worklog details for the
    /// given ids. Jira's documented per-call max is 1000; callers should batch.
    pub async fn worklog_list(&self, ids: &[i64]) -> Result<Vec<JiraWorklog>, JiraError> {
        let url = self.url("/rest/api/3/worklog/list")?;
        let body = json!({ "ids": ids });

        let resp = self
            .http
            .post(url)
            .basic_auth(&self.email, Some(&self.token))
            .json(&body)
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<Vec<JiraWorklog>>().await?)
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

        let resp = self
            .http
            .get(url)
            .basic_auth(&self.email, Some(&self.token))
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        Ok(resp.json::<IssueWorklogsPage>().await?)
    }
}

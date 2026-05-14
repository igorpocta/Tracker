use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::json;
use thiserror::Error;
use url::Url;

use super::adf::make_adf_comment;
use super::models::{JiraUser, SearchPage, WorklogResponse};

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
    /// page. Pass `None` for the first page.
    pub async fn search_jql(
        &self,
        jql: &str,
        page_token: Option<&str>,
        fields: &[&str],
    ) -> Result<SearchPage, JiraError> {
        let url = self.url("/rest/api/3/search/jql")?;
        let mut body = json!({
            "jql": jql,
            "fields": fields,
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
}

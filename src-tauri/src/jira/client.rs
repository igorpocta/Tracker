use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use thiserror::Error;
use url::Url;

use super::models::JiraUser;

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
}

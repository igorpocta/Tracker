//! Typed Freelo API client with HTTP Basic auth (email + API key).
//!
//! See `https://freelo.docs.apiary.io/` for the canonical schema. The exact
//! path layout is verified at runtime; this module's job is to render a
//! typed Rust surface that the rest of the app can call.

use std::future::Future;
use std::time::Duration;

use chrono::NaiveDate;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use thiserror::Error;
use url::Url;

use super::models::{FreeloProject, FreeloTask, FreeloUser, FreeloWorkReport};

/// Max retries after a 429.
pub const MAX_RETRIES: u32 = 3;
/// Cap on retry wait derived from `Retry-After` or backoff.
pub const MAX_RETRY_WAIT_SECS: u64 = 60;

/// Errors produced by the Freelo API client.
#[derive(Debug, Error)]
pub enum FreeloError {
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
    #[error("work report not found")]
    WorkReportNotFound,
    #[error("rate limited (retry after {retry_after_secs:?} seconds)")]
    RateLimited { retry_after_secs: Option<u64> },
}

/// HTTP Basic-authenticated client for the Freelo v1 API.
#[derive(Debug, Clone)]
pub struct FreeloClient {
    http: Client,
    base_url: Url,
    email: String,
    api_key: String,
}

async fn default_sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// Retry wrapper for Freelo calls — same shape as the Jira one but typed for
/// [`FreeloError::RateLimited`].
pub async fn with_retry<F, Fut, T>(f: F) -> Result<T, FreeloError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, FreeloError>>,
{
    with_retry_using(f, default_sleep).await
}

pub(crate) async fn with_retry_using<F, Fut, T, S, SFut>(
    mut f: F,
    sleep: S,
) -> Result<T, FreeloError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, FreeloError>>,
    S: Fn(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut attempt: u32 = 0;
    loop {
        match f().await {
            Err(FreeloError::RateLimited { retry_after_secs }) if attempt < MAX_RETRIES => {
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

fn parse_retry_after(h: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let v = h?;
    let s = v.to_str().ok()?.trim();
    s.parse::<u64>().ok()
}

impl FreeloClient {
    /// Build a new Freelo client.
    ///
    /// `base_url` defaults to [`super::DEFAULT_BASE_URL`] when empty (the
    /// caller's `FreeloConnectionConfig` already normalises this).
    pub fn new(base_url: String, email: String, api_key: String) -> Result<Self, FreeloError> {
        let url = if base_url.trim().is_empty() {
            super::DEFAULT_BASE_URL.to_string()
        } else {
            base_url
        };
        let base_url = Url::parse(&url)?;
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
            api_key,
        })
    }

    fn url(&self, path: &str) -> Result<Url, FreeloError> {
        // Path may or may not start with `/`. We normalise: strip leading
        // slashes and append to the base URL's path with a `/` separator.
        let trimmed = path.trim_start_matches('/');
        let mut base = self.base_url.clone();
        // Ensure base path ends with `/` so .join() appends rather than
        // replacing the last segment.
        if !base.path().ends_with('/') {
            let p = format!("{}/", base.path());
            base.set_path(&p);
        }
        Ok(base.join(trimmed)?)
    }

    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, FreeloError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(FreeloError::Unauthorized);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after_secs = parse_retry_after(resp.headers().get("Retry-After"));
            return Err(FreeloError::RateLimited { retry_after_secs });
        }
        let code = status.as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(FreeloError::Api { status: code, body })
    }

    /// `GET /users/me` — authentication health check.
    ///
    /// Per the official Freelo v1 docs this returns `{result: "success"}`
    /// (200) on valid credentials and 401 on bad ones. The response doesn't
    /// carry user-detail fields, so we synthesize a minimal `FreeloUser`
    /// from the supplied email (used downstream as the worklog author tag).
    pub async fn me(&self) -> Result<FreeloUser, FreeloError> {
        let url = self.url("/users/me")?;
        let _: Value = with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            // Body is `{result: "success"}` — we don't need to inspect it,
            // but consuming the JSON keeps the wire reusable.
            Ok(resp.json::<Value>().await.unwrap_or(Value::Null))
        })
        .await?;

        Ok(FreeloUser {
            id: 0,
            email: Some(self.email.clone()),
            display_name: Some(derive_name_from_email(&self.email)),
            first_name: None,
            last_name: None,
        })
    }

    /// `GET /all-projects` — list all projects the authenticated user has
    /// access to. Freelo returns project objects keyed by id; we flatten the
    /// response shape and return a typed list.
    pub async fn list_projects(&self) -> Result<Vec<FreeloProject>, FreeloError> {
        // Freelo `/all-projects` is paginated. Per the official v1 docs we
        // pass `p=N` (0-indexed) plus the canonical sort params. We loop
        // through pages until we've collected `total` projects or we get an
        // empty page.
        //
        // Canonical response shape:
        //   { total, count, page, per_page, data: { projects: [ ... ] } }
        //
        // The parser also tolerates older shapes for resilience:
        //   { projects: [ ... ] }                 — flat-keyed
        //   { owned_projects, invited_projects }  — earlier guess
        //   [ ... ]                               — bare array (legacy)
        let mut out: Vec<FreeloProject> = Vec::new();
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut page: u32 = 0;
        // Hard safety cap to avoid runaway loops on malformed responses.
        const MAX_PAGES: u32 = 50;

        loop {
            let mut url = self.url("/all-projects")?;
            url.query_pairs_mut()
                .append_pair("order_by", "date_add")
                .append_pair("order", "asc")
                .append_pair("p", &page.to_string());

            let body: Value = with_retry(|| async {
                let resp = self
                    .http
                    .get(url.clone())
                    .basic_auth(&self.email, Some(&self.api_key))
                    .send()
                    .await?;
                let resp = Self::check_status(resp).await?;
                Ok(resp.json::<Value>().await?)
            })
            .await?;

            let before = out.len();
            let mut push_arr = |arr: &Vec<Value>| {
                for v in arr {
                    if let Ok(p) = serde_json::from_value::<FreeloProject>(v.clone()) {
                        if seen.insert(p.id) {
                            out.push(p);
                        }
                    }
                }
            };
            if let Some(arr) = body
                .pointer("/data/projects")
                .and_then(|v| v.as_array())
            {
                push_arr(arr);
            } else if let Some(obj) = body.as_object() {
                for key in ["projects", "owned_projects", "invited_projects"] {
                    if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
                        push_arr(arr);
                    }
                }
            } else if let Some(arr) = body.as_array() {
                push_arr(arr);
            }

            // Decide whether to fetch the next page.
            let added = out.len() - before;
            let total = body.get("total").and_then(|v| v.as_u64()).map(|t| t as usize);
            // Stop when:
            //  • the response carried `total` and we've reached it, OR
            //  • this page added nothing (empty / non-paginated response), OR
            //  • we hit the safety cap.
            // Legacy shapes (bare array, no `total`) only ever return one
            // page, so `added == 0` naturally short-circuits after page 0.
            let reached_total = total.map(|t| out.len() >= t).unwrap_or(false);
            if added == 0 || reached_total || page + 1 >= MAX_PAGES {
                break;
            }
            // If there's no `total` (legacy shape), don't risk a second
            // request that will likely 404 — bail after the first page.
            if total.is_none() {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    /// `GET /project/{id}/tasks` — list all tasks in a project. Returns a
    /// flat list; Freelo's nested tasklist hierarchy is flattened so each
    /// task carries its parent `tasklist_id` when available.
    pub async fn list_tasks_for_project(
        &self,
        project_id: i64,
    ) -> Result<Vec<FreeloTask>, FreeloError> {
        let url = self.url(&format!("/project/{project_id}/all-tasks"))?;
        let body: Value = with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        let mut out = Vec::new();
        let arr_opt = if let Some(a) = body.as_array() {
            Some(a.clone())
        } else {
            // Sometimes Freelo nests under {"tasks": [...]} or
            // {"tasklists": [{"tasks": [...]}]}. Walk both.
            if let Some(tasks) = body.get("tasks").and_then(|v| v.as_array()) {
                Some(tasks.clone())
            } else if let Some(lists) = body.get("tasklists").and_then(|v| v.as_array()) {
                let mut acc: Vec<Value> = Vec::new();
                for tl in lists {
                    let tl_id = tl.get("id").and_then(|v| v.as_i64());
                    if let Some(tasks) = tl.get("tasks").and_then(|v| v.as_array()) {
                        for t in tasks {
                            let mut t = t.clone();
                            if let Some(id) = tl_id {
                                if let Some(obj) = t.as_object_mut() {
                                    obj.insert("tasklist_id".into(), json!(id));
                                }
                            }
                            acc.push(t);
                        }
                    }
                }
                Some(acc)
            } else {
                None
            }
        };

        if let Some(a) = arr_opt {
            for v in a {
                // Flatten {"project": {"id": N}} into a top-level project_id
                // so the FreeloTask deserialiser finds it.
                let mut v = v;
                if v.get("project_id").is_none() {
                    if let Some(pid) = v
                        .get("project")
                        .and_then(|p| p.get("id"))
                        .and_then(|v| v.as_i64())
                    {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("project_id".into(), json!(pid));
                        }
                    } else if let Some(obj) = v.as_object_mut() {
                        obj.insert("project_id".into(), json!(project_id));
                    }
                }
                if let Ok(t) = serde_json::from_value::<FreeloTask>(v) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    /// `GET /timesheets` — list work-reports in a date range, optionally
    /// filtered to a specific user. Freelo's endpoint accepts ISO dates for
    /// `date_from` / `date_to`.
    pub async fn list_work_reports(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        user_id: i64,
    ) -> Result<Vec<FreeloWorkReport>, FreeloError> {
        let mut url = self.url("/timesheets")?;
        url.query_pairs_mut()
            .append_pair("date_from", &from.format("%Y-%m-%d").to_string())
            .append_pair("date_to", &to.format("%Y-%m-%d").to_string())
            .append_pair("users_ids[]", &user_id.to_string());

        let body: Value = with_retry(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        let mut out = Vec::new();
        let arr_opt = if let Some(a) = body.as_array() {
            Some(a.clone())
        } else {
            body.get("work_reports")
                .or_else(|| body.get("timesheets"))
                .and_then(|v| v.as_array())
                .cloned()
        };

        if let Some(a) = arr_opt {
            for v in a {
                let mut v = v;
                // Flatten {"task": {"id": N}} into top-level `task_id`.
                if v.get("task_id").is_none() {
                    if let Some(tid) = v
                        .get("task")
                        .and_then(|t| t.get("id"))
                        .and_then(|x| x.as_i64())
                    {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("task_id".into(), json!(tid));
                        }
                    }
                }
                // Same for user.
                if v.get("user_id").is_none() {
                    if let Some(uid) = v
                        .get("worker")
                        .and_then(|u| u.get("id"))
                        .and_then(|x| x.as_i64())
                    {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("user_id".into(), json!(uid));
                        }
                    } else if let Some(uid) = v
                        .get("user")
                        .and_then(|u| u.get("id"))
                        .and_then(|x| x.as_i64())
                    {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("user_id".into(), json!(uid));
                        }
                    }
                }
                if let Ok(w) = serde_json::from_value::<FreeloWorkReport>(v) {
                    if w.user_id == user_id {
                        out.push(w);
                    }
                }
            }
        }
        Ok(out)
    }

    /// `POST /task/{id}/work-reports` — create a new work-report on a task.
    ///
    /// `date` is the date worked (Freelo only stores date, not time-of-day).
    /// `minutes` must be ≥ 1 — callers are responsible for catching the
    /// "round to 0" case before invoking.
    pub async fn create_work_report(
        &self,
        task_id: i64,
        date: NaiveDate,
        minutes: i64,
        description: Option<&str>,
    ) -> Result<FreeloWorkReport, FreeloError> {
        let url = self.url(&format!("/task/{task_id}/work-reports"))?;
        let date_s = date.format("%Y-%m-%d").to_string();
        let mut body = json!({
            "minutes": minutes,
            "date_reported": date_s,
        });
        if let Some(d) = description {
            if !d.trim().is_empty() {
                body["description"] = json!(d);
            }
        }
        let body_clone = body.clone();

        let value: Value = with_retry(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .json(&body_clone)
                .send()
                .await?;
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        // The response shape on Freelo is `{ "work_report": { … } }` on some
        // endpoints, or a top-level object on others. Flatten if needed.
        let mut v = value
            .get("work_report")
            .cloned()
            .unwrap_or(value);

        // Make sure `task_id` is present (Freelo sometimes omits it on the
        // create response since it's in the URL).
        if v.get("task_id").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("task_id".into(), json!(task_id));
            }
        }
        // Ensure date_reported is populated.
        if v.get("date_reported").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("date_reported".into(), json!(date_s));
            }
        }
        // Same for minutes.
        if v.get("minutes").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("minutes".into(), json!(minutes));
            }
        }

        serde_json::from_value::<FreeloWorkReport>(v).map_err(FreeloError::Serde)
    }

    /// `POST /work-reports/{id}` — update an existing work-report.
    ///
    /// Freelo's API uses POST (not PUT) for updates on some endpoints. We
    /// follow the documented surface.
    pub async fn update_work_report(
        &self,
        work_report_id: i64,
        minutes: Option<i64>,
        date: Option<NaiveDate>,
        description: Option<&str>,
    ) -> Result<FreeloWorkReport, FreeloError> {
        let url = self.url(&format!("/work-reports/{work_report_id}"))?;
        let mut body = serde_json::Map::new();
        if let Some(m) = minutes {
            body.insert("minutes".into(), json!(m));
        }
        if let Some(d) = date {
            body.insert(
                "date_reported".into(),
                json!(d.format("%Y-%m-%d").to_string()),
            );
        }
        if let Some(d) = description {
            body.insert("description".into(), json!(d));
        }
        let body = Value::Object(body);
        let body_clone = body.clone();

        let value: Value = with_retry(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .json(&body_clone)
                .send()
                .await?;
            let status = resp.status();
            if status == StatusCode::NOT_FOUND {
                return Err(FreeloError::WorkReportNotFound);
            }
            let resp = Self::check_status(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        let v = value.get("work_report").cloned().unwrap_or(value);
        serde_json::from_value::<FreeloWorkReport>(v).map_err(FreeloError::Serde)
    }

    /// `DELETE /work-reports/{id}` — remove a work-report.
    pub async fn delete_work_report(&self, work_report_id: i64) -> Result<(), FreeloError> {
        let url = self.url(&format!("/work-reports/{work_report_id}"))?;
        with_retry(|| async {
            let resp = self
                .http
                .delete(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let status = resp.status();
            if status == StatusCode::NO_CONTENT || status.is_success() {
                return Ok(());
            }
            if status == StatusCode::NOT_FOUND {
                return Err(FreeloError::WorkReportNotFound);
            }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(FreeloError::Unauthorized);
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after_secs = parse_retry_after(resp.headers().get("Retry-After"));
                return Err(FreeloError::RateLimited { retry_after_secs });
            }
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(FreeloError::Api { status: code, body })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn with_retry_propagates_non_429() {
        let err: FreeloError = with_retry(|| async {
            Err::<(), _>(FreeloError::Unauthorized)
        })
        .await
        .unwrap_err();
        assert!(matches!(err, FreeloError::Unauthorized));
    }

    #[tokio::test]
    async fn with_retry_succeeds_after_one_429() {
        let mut attempts: u32 = 0;
        let res: Result<u32, FreeloError> = with_retry_using(
            || {
                attempts += 1;
                async move {
                    if attempts == 1 {
                        Err(FreeloError::RateLimited {
                            retry_after_secs: Some(0),
                        })
                    } else {
                        Ok(attempts)
                    }
                }
            },
            |_| async {},
        )
        .await;
        assert_eq!(res.unwrap(), 2);
    }
}

/// Heuristic display name from an email: `igor.pocta@example.com` → `Igor Pocta`.
fn derive_name_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    local
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

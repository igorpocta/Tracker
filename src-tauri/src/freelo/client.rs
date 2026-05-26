//! Typed Freelo API client with HTTP Basic auth (email + API key).
//!
//! See `https://freelo.docs.apiary.io/` for the canonical schema. The exact
//! path layout is verified at runtime; this module's job is to render a
//! typed Rust surface that the rest of the app can call.

use chrono::{DateTime, FixedOffset, NaiveDate, SecondsFormat};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use thiserror::Error;
use url::Url;

use super::models::{FreeloProject, FreeloTask, FreeloUser, FreeloWorkReport};
use crate::http_base::{self, HttpError, RateLimitInfo};

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

impl HttpError for FreeloError {
    fn as_rate_limit(&self) -> Option<RateLimitInfo> {
        if let FreeloError::RateLimited { retry_after_secs } = self {
            Some(RateLimitInfo {
                retry_after_secs: *retry_after_secs,
            })
        } else {
            None
        }
    }
    fn rate_limited(retry_after_secs: Option<u64>) -> Self {
        FreeloError::RateLimited { retry_after_secs }
    }
    fn unauthorized() -> Self {
        FreeloError::Unauthorized
    }
    fn api(status: u16, body: String) -> Self {
        FreeloError::Api { status, body }
    }
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

    /// `GET /users/me` — authentication health check + minimal user info.
    ///
    /// Per the official Freelo v1 docs (verified against the live API), the
    /// response is `{ result: "success", user: { id: N } }` on 200, and 401
    /// on bad credentials. We pull `user.id` so worklog filters can scope to
    /// the caller without an extra round-trip; full name + email are still
    /// derived from the supplied email since the endpoint doesn't return them.
    pub async fn me(&self) -> Result<FreeloUser, FreeloError> {
        let url = self.url("/users/me")?;
        let body: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let resp = http_base::check_status::<FreeloError>(resp).await?;
            Ok(resp.json::<Value>().await.unwrap_or(Value::Null))
        })
        .await?;

        let id = body
            .pointer("/user/id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(FreeloUser {
            id,
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

            let body: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
                let resp = self
                    .http
                    .get(url.clone())
                    .basic_auth(&self.email, Some(&self.api_key))
                    .send()
                    .await?;
                let resp = http_base::check_status::<FreeloError>(resp).await?;
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
            if let Some(arr) = body.pointer("/data/projects").and_then(|v| v.as_array()) {
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
            let total = body
                .get("total")
                .and_then(|v| v.as_u64())
                .map(|t| t as usize);
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
        let body: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
            let resp = self
                .http
                .get(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .send()
                .await?;
            let resp = http_base::check_status::<FreeloError>(resp).await?;
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
                if v.get("project_name").is_none() {
                    if let Some(name) = v
                        .get("project")
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                    {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("project_name".into(), json!(name));
                        }
                    }
                }
                if let Ok(t) = serde_json::from_value::<FreeloTask>(v) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    /// `GET /all-tasks?projects_ids[]=X&p=N` — global paginated task search
    /// filtered to a set of project ids. Per the official v1 docs:
    /// <https://api.freelo.io/docs/v1/freelo-api>.
    ///
    /// Response shape:
    ///   `{ total, count, page, per_page, data: { tasks: [ ... ] } }`
    /// Each task carries `project: { id, name, state }`, `state: { id, state }`,
    /// `worker: { id, name }`, `tasklist: { id, name, state }`. We flatten the
    /// nested project info into `project_id` + `project_name` so the model can
    /// deserialize cleanly.
    pub async fn list_tasks_for_projects(
        &self,
        project_ids: &[i64],
    ) -> Result<Vec<FreeloTask>, FreeloError> {
        let mut out: Vec<FreeloTask> = Vec::new();
        let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
        if project_ids.is_empty() {
            return Ok(out);
        }
        let mut page: u32 = 0;
        const MAX_PAGES: u32 = 200;
        loop {
            let mut url = self.url("/all-tasks")?;
            {
                let mut q = url.query_pairs_mut();
                q.append_pair("p", &page.to_string());
                for pid in project_ids {
                    q.append_pair("projects_ids[]", &pid.to_string());
                }
            }
            let body: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
                let resp = self
                    .http
                    .get(url.clone())
                    .basic_auth(&self.email, Some(&self.api_key))
                    .send()
                    .await?;
                let resp = http_base::check_status::<FreeloError>(resp).await?;
                Ok(resp.json::<Value>().await?)
            })
            .await?;

            let before = out.len();
            let arr = body
                .pointer("/data/tasks")
                .and_then(|v| v.as_array())
                .or_else(|| body.get("tasks").and_then(|v| v.as_array()))
                .or_else(|| body.as_array());
            if let Some(arr) = arr {
                for v in arr {
                    let mut v = v.clone();
                    // Flatten {project: {id, name}} into top-level fields the
                    // deserializer understands.
                    let nested_project = v.get("project").cloned();
                    if let Some(np) = nested_project.as_ref() {
                        if v.get("project_id").is_none() {
                            if let Some(pid) = np.get("id").and_then(|x| x.as_i64()) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("project_id".into(), json!(pid));
                                }
                            }
                        }
                        if v.get("project_name").is_none() {
                            if let Some(pname) = np.get("name").and_then(|x| x.as_str()) {
                                if let Some(obj) = v.as_object_mut() {
                                    obj.insert("project_name".into(), json!(pname));
                                }
                            }
                        }
                    }
                    if let Ok(t) = serde_json::from_value::<FreeloTask>(v) {
                        if seen_ids.insert(t.id) {
                            out.push(t);
                        }
                    }
                }
            }
            let added = out.len() - before;
            let total = body
                .get("total")
                .and_then(|v| v.as_u64())
                .map(|t| t as usize);
            let reached_total = total.map(|t| out.len() >= t).unwrap_or(false);
            if added == 0 || reached_total || page + 1 >= MAX_PAGES {
                break;
            }
            if total.is_none() {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    /// `GET /work-reports` — paginated list of work reports filtered by
    /// projects + user. Per the official v1 docs this is the canonical
    /// "list time entries" endpoint.
    ///
    /// We pass:
    ///   - `projects_ids[]` for each selected project (limits the scope to
    ///     projects the user has chosen to track in this app)
    ///   - `users_ids[]` for the authenticated user (so we never see other
    ///     people's entries even on shared projects)
    ///   - `date_from` / `date_to` to limit the range
    ///
    /// Response shape is the canonical PaginatedResponse:
    ///   `{ total, count, page, per_page, data: { reports: [ ... ] } }`
    /// where each report carries nested `task: {id, name}`, `project: {id,
    /// name}`, `author: {id}`, `worker: {id}`, `date_reported` (ISO 8601 with
    /// TZ), and `note` for the comment. We flatten those into the flat fields
    /// `FreeloWorkReport` expects.
    pub async fn list_work_reports(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        user_id: i64,
        project_ids: &[i64],
    ) -> Result<Vec<FreeloWorkReport>, FreeloError> {
        let mut out: Vec<FreeloWorkReport> = Vec::new();
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut page: u32 = 0;
        const MAX_PAGES: u32 = 200;
        loop {
            let mut url = self.url("/work-reports")?;
            {
                let mut q = url.query_pairs_mut();
                // Per Freelo v1 docs: filtr na `date_reported` se předává jako
                // `date_reported_range[date_from]` / `[date_to]`. Předchozí
                // `date_from` / `date_to` server ignoroval a vracel default
                // (vše), takže klient pak musel filtrovat lokálně.
                q.append_pair(
                    "date_reported_range[date_from]",
                    &from.format("%Y-%m-%d").to_string(),
                )
                .append_pair(
                    "date_reported_range[date_to]",
                    &to.format("%Y-%m-%d").to_string(),
                )
                .append_pair("p", &page.to_string());
                if user_id > 0 {
                    q.append_pair("users_ids[]", &user_id.to_string());
                }
                for pid in project_ids {
                    q.append_pair("projects_ids[]", &pid.to_string());
                }
            }
            let body: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
                let resp = self
                    .http
                    .get(url.clone())
                    .basic_auth(&self.email, Some(&self.api_key))
                    .send()
                    .await?;
                let resp = http_base::check_status::<FreeloError>(resp).await?;
                Ok(resp.json::<Value>().await?)
            })
            .await?;

            let before = out.len();
            let arr = body
                .pointer("/data/reports")
                .and_then(|v| v.as_array())
                .or_else(|| body.get("reports").and_then(|v| v.as_array()))
                .or_else(|| body.get("work_reports").and_then(|v| v.as_array()))
                .or_else(|| body.as_array());
            if let Some(arr) = arr {
                for v in arr {
                    let mut v = v.clone();
                    flatten_work_report_fields(&mut v);
                    if let Ok(w) = serde_json::from_value::<FreeloWorkReport>(v) {
                        // Defensive client-side filter: even if the server
                        // ignored our users_ids[] filter, never let another
                        // user's reports leak into our cache.
                        if user_id > 0 && w.user_id != user_id {
                            continue;
                        }
                        if seen.insert(w.id) {
                            out.push(w);
                        }
                    }
                }
            }
            let added = out.len() - before;
            let total = body
                .get("total")
                .and_then(|v| v.as_u64())
                .map(|t| t as usize);
            let reached_total = total.map(|t| out.len() >= t).unwrap_or(false);
            if added == 0 || reached_total || page + 1 >= MAX_PAGES {
                break;
            }
            if total.is_none() {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    /// `POST /task/{id}/work-reports` — create a new work-report on a task.
    ///
    /// `started_at` je plný okamžik začátku seance — Freelo `date_reported`
    /// bere RFC3339 timestamp s TZ a uloží ho, jak ho pošleme (nezahazuje
    /// time-of-day). `minutes` musí být ≥ 1 — volající je zodpovědný za
    /// odchycení "round to 0".
    pub async fn create_work_report(
        &self,
        task_id: i64,
        started_at: DateTime<FixedOffset>,
        minutes: i64,
        description: Option<&str>,
    ) -> Result<FreeloWorkReport, FreeloError> {
        let url = self.url(&format!("/task/{task_id}/work-reports"))?;
        let started_s = started_at.to_rfc3339_opts(SecondsFormat::Secs, false);
        let mut body = json!({
            "minutes": minutes,
            "date_reported": started_s,
        });
        // Pole se na Freelo API jmenuje `note` — ne `description`. Bez tohoto
        // se uživatelská poznámka tiše ztrácela mezi naší DB a Freelem.
        if let Some(d) = description {
            if !d.trim().is_empty() {
                body["note"] = json!(d);
            }
        }
        let body_clone = body.clone();

        let value: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
            let resp = self
                .http
                .post(url.clone())
                .basic_auth(&self.email, Some(&self.api_key))
                .json(&body_clone)
                .send()
                .await?;
            let resp = http_base::check_status::<FreeloError>(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        // The response shape on Freelo is `{ "work_report": { … } }` on some
        // endpoints, or a top-level object on others. Flatten if needed.
        let mut v = value.get("work_report").cloned().unwrap_or(value);

        // Use the same flattener as the GET /work-reports list response so
        // nested task.id / author.id / worker.id / note get hoisted to the
        // flat fields FreeloWorkReport deserializes.
        flatten_work_report_fields(&mut v);

        // Defaults for fields Freelo sometimes omits on the create response
        // (they're either in the request URL or echoed back inconsistently).
        if v.get("task_id").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("task_id".into(), json!(task_id));
            }
        }
        if v.get("date_reported").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("date_reported".into(), json!(started_s));
            }
        }
        if v.get("minutes").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("minutes".into(), json!(minutes));
            }
        }
        // Freelo's create response sometimes omits author/worker entirely
        // (the API knows the caller — no need to echo). Default user_id to 0
        // so the local row gets saved; the next /work-reports sync will
        // overwrite this with the authoritative value.
        if v.get("user_id").is_none() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("user_id".into(), json!(0));
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
        started_at: Option<DateTime<FixedOffset>>,
        description: Option<&str>,
    ) -> Result<FreeloWorkReport, FreeloError> {
        let url = self.url(&format!("/work-reports/{work_report_id}"))?;
        let mut body = serde_json::Map::new();
        if let Some(m) = minutes {
            body.insert("minutes".into(), json!(m));
        }
        if let Some(dt) = started_at {
            body.insert(
                "date_reported".into(),
                json!(dt.to_rfc3339_opts(SecondsFormat::Secs, false)),
            );
        }
        if let Some(d) = description {
            // Stejný důvod jako v `create_work_report` — Freelo API očekává
            // `note`, ne `description`.
            body.insert("note".into(), json!(d));
        }
        let body = Value::Object(body);
        let body_clone = body.clone();

        let value: Value = http_base::with_retry::<_, _, _, FreeloError>(|| async {
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
            let resp = http_base::check_status::<FreeloError>(resp).await?;
            Ok(resp.json::<Value>().await?)
        })
        .await?;

        let v = value.get("work_report").cloned().unwrap_or(value);
        serde_json::from_value::<FreeloWorkReport>(v).map_err(FreeloError::Serde)
    }

    /// `DELETE /work-reports/{id}` — remove a work-report.
    pub async fn delete_work_report(&self, work_report_id: i64) -> Result<(), FreeloError> {
        let url = self.url(&format!("/work-reports/{work_report_id}"))?;
        http_base::with_retry::<_, _, _, FreeloError>(|| async {
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
                let retry_after_secs =
                    http_base::parse_retry_after(resp.headers().get("Retry-After"));
                return Err(FreeloError::RateLimited { retry_after_secs });
            }
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(FreeloError::Api { status: code, body })
        })
        .await
    }
}

/// Flatten the canonical `/work-reports` response shape into the flat fields
/// `FreeloWorkReport` deserializes. Inserts:
///   - `task_id`   ← `task.id`
///   - `task_name` ← `task.name`
///   - `user_id`   ← `author.id` (preferred) or `worker.id`
///   - `description` ← `note`
fn flatten_work_report_fields(v: &mut Value) {
    if v.get("task_id").is_none() {
        if let Some(tid) = v.pointer("/task/id").and_then(|x| x.as_i64()) {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("task_id".into(), json!(tid));
            }
        }
    }
    if v.get("task_name").is_none() {
        if let Some(name) = v
            .pointer("/task/name")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
        {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("task_name".into(), json!(name));
            }
        }
    }
    if v.get("user_id").is_none() {
        let uid = v
            .pointer("/author/id")
            .and_then(|x| x.as_i64())
            .or_else(|| v.pointer("/worker/id").and_then(|x| x.as_i64()))
            .or_else(|| v.pointer("/user/id").and_then(|x| x.as_i64()));
        if let Some(uid) = uid {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("user_id".into(), json!(uid));
            }
        }
    }
    if v.get("description").is_none() {
        if let Some(note) = v.get("note").cloned() {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("description".into(), note);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke-test that `FreeloError` plugs into the shared retry primitives
    /// correctly: non-429 errors should bubble out on the first attempt.
    /// The retry-loop's behaviour itself is covered in `http_base::tests`.
    #[tokio::test]
    async fn freelo_error_propagates_non_429_through_retry() {
        let err: FreeloError =
            http_base::with_retry(|| async { Err::<(), _>(FreeloError::Unauthorized) })
                .await
                .unwrap_err();
        assert!(matches!(err, FreeloError::Unauthorized));
    }

    /// Smoke-test that `FreeloError::RateLimited` triggers a retry that
    /// eventually succeeds. The exhaustive retry behaviour (clamps,
    /// exponential backoff, max-retries) lives in `http_base::tests`.
    #[tokio::test]
    async fn freelo_rate_limit_triggers_retry_via_http_base() {
        let mut attempts: u32 = 0;
        let res: Result<u32, FreeloError> = http_base::with_retry_using(
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

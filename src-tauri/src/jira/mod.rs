//! Jira Cloud REST API v3 client.
//!
//! Provides a typed wrapper around the endpoints used by Tracker:
//! - `GET  /rest/api/3/myself`
//! - `POST /rest/api/3/search/jql`
//! - `POST /rest/api/3/issue/{key}/worklog`
//!
//! The client uses `reqwest` with `rustls-tls` and Basic auth (email + API token).

pub mod client;
pub mod models;

pub use client::{JiraClient, JiraError};
pub use models::JiraUser;

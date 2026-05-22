//! `WorklogService` implementation for Freelo.
//!
//! Phase B4 — Freelo's sync needs both the [`FreeloClient`] and the
//! per-connection `FreeloConnectionConfig` (project selection, cached
//! user id). Instead of teaching the trait about config-bearing
//! providers, we wrap both in a [`FreeloService`] struct that the
//! `ProviderClient::Freelo` variant now holds.
//!
//! Call sites that previously destructured the variant as
//! `ProviderClient::Freelo(client, cfg)` now go through
//! `ProviderClient::Freelo(svc)` and access `svc.client` / `svc.config`.

use chrono::NaiveDate;

use super::client::FreeloClient;
use crate::cache::Db;
use crate::commands::connections::FreeloConnectionConfig;
use crate::worklog_service::{ServiceFuture, SyncOutcome, WorklogService};

/// A built Freelo connection: the HTTP client plus the persisted config
/// (selected projects, cached user id). One per `ProviderClient::Freelo`.
#[derive(Debug, Clone)]
pub struct FreeloService {
    pub client: FreeloClient,
    pub config: FreeloConnectionConfig,
}

impl FreeloService {
    /// Build a new service from an existing client + config pair.
    pub fn new(client: FreeloClient, config: FreeloConnectionConfig) -> Self {
        Self { client, config }
    }
}

impl WorklogService for FreeloService {
    fn provider_name(&self) -> &'static str {
        "freelo"
    }

    fn sync_issues<'a>(&'a self, db: &'a Db, conn_id: i64) -> ServiceFuture<'a, usize> {
        Box::pin(async move {
            let n = crate::freelo::sync::sync_issues_for_connection(
                &self.client,
                db,
                conn_id,
                &self.config.selected_project_ids,
            )
            .await?;
            Ok(n)
        })
    }

    fn sync_worklogs<'a>(
        &'a self,
        db: &'a Db,
        conn_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> ServiceFuture<'a, SyncOutcome> {
        Box::pin(async move {
            // Readiness gate: Freelo's worklog endpoint needs an explicit
            // `user_id`. We cache it on the config at sync time; when the
            // connection has never resolved it (first run, or hand-rolled
            // import) we surface Skipped so the orchestrator keeps the
            // persisted error and the UI sees a warning — short-circuiting
            // to Ok(0) used to silently fake a healthy sync.
            let Some(user_id) = self.config.sync_user_id else {
                return Ok(SyncOutcome::skipped(
                    "freelo: chybí sync_user_id (spusťte inicializaci syncu)".to_string(),
                ));
            };
            let n = crate::freelo::sync::sync_worklogs_for_range(
                &self.client,
                db,
                conn_id,
                user_id,
                from,
                to,
                &self.config.selected_project_ids,
            )
            .await?;
            Ok(SyncOutcome::ok(n))
        })
    }
}

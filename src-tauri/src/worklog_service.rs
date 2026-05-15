//! Trait abstraction over per-provider sync operations.
//!
//! Phase B4 — the per-connection sync loop (`sync_one_connection` in
//! `commands::worklog::sync`) used to `match` on `ProviderClient` and run
//! two near-identical bodies (one per provider). The bodies differed only
//! in *which* client object they called and what extra config they passed.
//!
//! The [`WorklogService`] trait extracts that shape: every provider gets
//! one implementation, and the dispatch site calls it through `&dyn
//! WorklogService`. Adding Toggl / Clockify / … in the future means
//! `impl WorklogService for TogglClient {}` plus a new `ProviderClient`
//! variant — no further edits to the sync orchestrator.
//!
//! ## Why hand-rolled `Pin<Box<dyn Future>>` instead of `async_trait`?
//!
//! The trait has two `async` methods. Stable Rust does not let us write
//! `async fn` directly in a trait object–safe trait, so we'd normally
//! reach for the [`async_trait`] crate. We're deliberately *not* doing
//! that here:
//!
//! * It would add a new top-level dep to `Cargo.toml`.
//! * The macro hides the future type, which makes the trait harder to
//!   reason about when you're trying to figure out what `Send` bound the
//!   returned future actually carries.
//! * The hand-rolled `Pin<Box<dyn Future<Output = …> + Send + 'a>>`
//!   signature is verbose at the declaration site but trivial to call
//!   (`svc.sync_issues(db, id).await`) and forces each impl to be honest
//!   about lifetimes / `Send`-ness.

use chrono::NaiveDate;

use crate::cache::Db;

/// Future returned by [`WorklogService`] methods.
pub type ServiceFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>>;

/// Per-provider sync surface. One implementation per provider client.
///
/// All methods return the count of rows the call affected (issues
/// upserted / worklogs upserted). Errors are wrapped in [`anyhow::Error`]
/// so the dispatch site can stringify them uniformly without leaking
/// provider-specific error types into the orchestrator.
pub trait WorklogService: Send + Sync {
    /// Stable provider tag used in user-facing audit rows and progress
    /// events (e.g. `"jira"`, `"freelo"`).
    fn provider_name(&self) -> &'static str;

    /// Pull the issue catalog for this connection into the local cache.
    /// Returns the number of issues upserted.
    fn sync_issues<'a>(&'a self, db: &'a Db, conn_id: i64) -> ServiceFuture<'a, usize>;

    /// Pull all worklogs in the inclusive `[from, to]` window into the
    /// local cache, applying the provider's mark-and-sweep semantics for
    /// rows that disappeared upstream. Returns the number of worklogs
    /// upserted.
    fn sync_worklogs<'a>(
        &'a self,
        db: &'a Db,
        conn_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> ServiceFuture<'a, usize>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Mock service that records how many times each method was called.
    /// Used to drive the trait through tests without needing a live HTTP
    /// stack or AppState.
    struct MockService {
        name: &'static str,
        issue_calls: Arc<AtomicUsize>,
        worklog_calls: Arc<AtomicUsize>,
        issues_to_return: usize,
        worklogs_to_return: usize,
    }

    impl WorklogService for MockService {
        fn provider_name(&self) -> &'static str {
            self.name
        }

        fn sync_issues<'a>(&'a self, _db: &'a Db, _conn_id: i64) -> ServiceFuture<'a, usize> {
            Box::pin(async move {
                self.issue_calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.issues_to_return)
            })
        }

        fn sync_worklogs<'a>(
            &'a self,
            _db: &'a Db,
            _conn_id: i64,
            _from: NaiveDate,
            _to: NaiveDate,
        ) -> ServiceFuture<'a, usize> {
            Box::pin(async move {
                self.worklog_calls.fetch_add(1, Ordering::SeqCst);
                Ok(self.worklogs_to_return)
            })
        }
    }

    fn open_inmem_db() -> Db {
        // The mock impls don't actually touch the DB, but we still need a
        // value of the right type to feed the trait signature.
        let tmp = tempfile::tempdir().expect("tempdir");
        Db::open(&tmp.path().join("t.db")).expect("open db")
    }

    #[tokio::test]
    async fn dispatch_invokes_sync_issues_through_trait() {
        let calls_i = Arc::new(AtomicUsize::new(0));
        let calls_w = Arc::new(AtomicUsize::new(0));
        let svc: Box<dyn WorklogService> = Box::new(MockService {
            name: "mock",
            issue_calls: calls_i.clone(),
            worklog_calls: calls_w.clone(),
            issues_to_return: 7,
            worklogs_to_return: 0,
        });
        let db = open_inmem_db();

        let n = svc.sync_issues(&db, 42).await.expect("ok");
        assert_eq!(n, 7);
        assert_eq!(calls_i.load(Ordering::SeqCst), 1);
        assert_eq!(calls_w.load(Ordering::SeqCst), 0);
        assert_eq!(svc.provider_name(), "mock");
    }

    #[tokio::test]
    async fn dispatch_invokes_sync_worklogs_through_trait() {
        let calls_i = Arc::new(AtomicUsize::new(0));
        let calls_w = Arc::new(AtomicUsize::new(0));
        let svc: Box<dyn WorklogService> = Box::new(MockService {
            name: "mock",
            issue_calls: calls_i.clone(),
            worklog_calls: calls_w.clone(),
            issues_to_return: 0,
            worklogs_to_return: 13,
        });
        let db = open_inmem_db();
        let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();

        let n = svc.sync_worklogs(&db, 42, from, to).await.expect("ok");
        assert_eq!(n, 13);
        assert_eq!(calls_w.load(Ordering::SeqCst), 1);
        assert_eq!(calls_i.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn typical_orchestration_calls_both_methods_once() {
        let calls_i = Arc::new(AtomicUsize::new(0));
        let calls_w = Arc::new(AtomicUsize::new(0));
        let svc = MockService {
            name: "mock",
            issue_calls: calls_i.clone(),
            worklog_calls: calls_w.clone(),
            issues_to_return: 3,
            worklogs_to_return: 5,
        };
        let db = open_inmem_db();
        let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();

        // Mimic the order sync_one_connection uses.
        let r1 = svc.sync_issues(&db, 1).await.expect("ok");
        let r2 = svc.sync_worklogs(&db, 1, from, to).await.expect("ok");

        assert_eq!(r1, 3);
        assert_eq!(r2, 5);
        assert_eq!(calls_i.load(Ordering::SeqCst), 1);
        assert_eq!(calls_w.load(Ordering::SeqCst), 1);
    }
}

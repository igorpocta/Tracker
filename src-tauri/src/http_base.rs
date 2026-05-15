//! Shared HTTP retry + status mapping primitives used by every provider
//! client (Jira, Freelo, …).
//!
//! Each provider defines its own typed error enum (`JiraError`, `FreeloError`)
//! because the API surfaces diverge. The retry loop, however, is identical:
//! we back off on rate-limits and surface everything else immediately. This
//! module owns that loop, plus the helpers for parsing `Retry-After` and
//! turning a `reqwest::Response` into a `Result` keyed off the
//! per-provider error type.
//!
//! Wiring a provider error type means implementing [`HttpError`] for it:
//!
//! ```ignore
//! impl http_base::HttpError for MyError {
//!     fn as_rate_limit(&self) -> Option<http_base::RateLimitInfo> {
//!         if let MyError::RateLimited { retry_after_secs } = self {
//!             Some(http_base::RateLimitInfo { retry_after_secs: *retry_after_secs })
//!         } else {
//!             None
//!         }
//!     }
//!     fn rate_limited(retry_after_secs: Option<u64>) -> Self { ... }
//!     fn unauthorized() -> Self { ... }
//!     fn api(status: u16, body: String) -> Self { ... }
//! }
//! ```

use std::future::Future;
use std::time::Duration;

use reqwest::header::HeaderValue;
use reqwest::StatusCode;

/// Maximum number of automatic retries after a 429 response before the error
/// is surfaced to the caller. Matches the per-provider constants the
/// pre-refactor clients shipped with.
pub const MAX_RETRIES: u32 = 3;

/// Hard cap on the wait derived from `Retry-After` (or the exponential
/// backoff fallback). Guards against a misbehaving server forcing us to
/// freeze the UI for minutes.
pub const MAX_RETRY_WAIT_SECS: u64 = 60;

/// Per-occurrence rate-limit hint returned by [`HttpError::as_rate_limit`].
///
/// `retry_after_secs == None` is the "rate-limited but no header" case —
/// the retry loop falls back to exponential backoff. `Some(n)` means the
/// server told us to wait exactly `n` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitInfo {
    pub retry_after_secs: Option<u64>,
}

/// The retry loop and status-mapper need three things from a provider error
/// type: a way to recognise a rate-limit hit, and constructors for the two
/// status-derived variants ("unauthorized" and "generic API error"). Anything
/// else stays inside the provider module — this trait is the minimum surface
/// `http_base` has to know.
pub trait HttpError: std::error::Error + Send + Sync + 'static {
    fn as_rate_limit(&self) -> Option<RateLimitInfo>;
    fn rate_limited(retry_after_secs: Option<u64>) -> Self;
    fn unauthorized() -> Self;
    fn api(status: u16, body: String) -> Self;
}

async fn default_sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// Production retry wrapper. Calls `f` up to [`MAX_RETRIES`] + 1 times,
/// waiting between attempts whenever the error reports itself as a
/// rate-limit. Any other error is returned on first occurrence.
pub async fn with_retry<F, Fut, T, E>(f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: HttpError,
{
    with_retry_using(f, default_sleep).await
}

/// Test-friendly variant that lets the caller plug in a stub sleep.
/// Production code uses [`with_retry`].
pub async fn with_retry_using<F, Fut, T, E, S, SFut>(mut f: F, sleep: S) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: HttpError,
    S: Fn(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut attempt: u32 = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => match e.as_rate_limit() {
                Some(info) if attempt < MAX_RETRIES => {
                    let wait = info
                        .retry_after_secs
                        .unwrap_or_else(|| 2u64.saturating_pow(attempt))
                        .min(MAX_RETRY_WAIT_SECS);
                    sleep(Duration::from_secs(wait)).await;
                    attempt += 1;
                }
                _ => return Err(e),
            },
        }
    }
}

/// Parse a `Retry-After` header. Only the seconds-integer form is supported
/// (`Retry-After: 30`). HTTP-date form is intentionally rejected — every
/// provider we integrate with emits seconds, and accepting dates would be
/// dead code we'd have to maintain.
pub fn parse_retry_after(h: Option<&HeaderValue>) -> Option<u64> {
    let v = h?;
    let s = v.to_str().ok()?.trim();
    s.parse::<u64>().ok()
}

/// Map a `reqwest::Response` into `Result<Response, E>` using the standard
/// status semantics every API client in this app shares: success passes
/// through, 401/403 maps to [`HttpError::unauthorized`], 429 maps to
/// [`HttpError::rate_limited`] with the parsed `Retry-After`, everything
/// else maps to [`HttpError::api`] with the body text.
pub async fn check_status<E: HttpError>(resp: reqwest::Response) -> Result<reqwest::Response, E> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(E::unauthorized());
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = parse_retry_after(resp.headers().get("Retry-After"));
        return Err(E::rate_limited(retry_after_secs));
    }
    let code = status.as_u16();
    let body = resp.text().await.unwrap_or_default();
    Err(E::api(code, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::time::Duration;
    use thiserror::Error;

    #[derive(Debug, Error, PartialEq, Eq)]
    enum TestError {
        #[error("rate limited (retry_after={0:?})")]
        RateLimited(Option<u64>),
        #[error("unauthorized")]
        Unauthorized,
        #[error("api: {0} {1}")]
        Api(u16, String),
    }

    impl HttpError for TestError {
        fn as_rate_limit(&self) -> Option<RateLimitInfo> {
            match self {
                TestError::RateLimited(secs) => Some(RateLimitInfo {
                    retry_after_secs: *secs,
                }),
                _ => None,
            }
        }
        fn rate_limited(retry_after_secs: Option<u64>) -> Self {
            TestError::RateLimited(retry_after_secs)
        }
        fn unauthorized() -> Self {
            TestError::Unauthorized
        }
        fn api(status: u16, body: String) -> Self {
            TestError::Api(status, body)
        }
    }

    /// Records every sleep duration the retry loop requests. Each `tokio`
    /// `sleep` call invokes this without actually delaying — tests stay
    /// instant.
    fn recording_sleep<'a>(
        log: &'a RefCell<Vec<Duration>>,
    ) -> impl Fn(Duration) -> std::future::Ready<()> + 'a {
        |d| {
            log.borrow_mut().push(d);
            std::future::ready(())
        }
    }

    #[tokio::test]
    async fn ok_first_try_returns_immediately() {
        let sleeps = RefCell::new(Vec::new());
        let res: Result<u32, TestError> = with_retry_using(
            || async { Ok::<u32, TestError>(42) },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(res, Ok(42));
        assert!(sleeps.borrow().is_empty());
    }

    #[tokio::test]
    async fn non_rate_limit_error_does_not_retry() {
        let sleeps = RefCell::new(Vec::new());
        let res: Result<u32, TestError> = with_retry_using(
            || async { Err::<u32, _>(TestError::Unauthorized) },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(res, Err(TestError::Unauthorized));
        assert!(sleeps.borrow().is_empty());
    }

    #[tokio::test]
    async fn rate_limit_with_retry_after_uses_header_value() {
        let attempts = RefCell::new(0u32);
        let sleeps = RefCell::new(Vec::new());
        let res: Result<u32, TestError> = with_retry_using(
            || {
                let n = *attempts.borrow();
                *attempts.borrow_mut() += 1;
                async move {
                    if n == 0 {
                        Err(TestError::RateLimited(Some(7)))
                    } else {
                        Ok(99)
                    }
                }
            },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(res, Ok(99));
        assert_eq!(sleeps.borrow().as_slice(), &[Duration::from_secs(7)]);
    }

    #[tokio::test]
    async fn rate_limit_without_header_uses_exponential_backoff() {
        let attempts = RefCell::new(0u32);
        let sleeps = RefCell::new(Vec::new());
        let res: Result<u32, TestError> = with_retry_using(
            || {
                let n = *attempts.borrow();
                *attempts.borrow_mut() += 1;
                async move {
                    if n < 3 {
                        Err(TestError::RateLimited(None))
                    } else {
                        Ok(1)
                    }
                }
            },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(res, Ok(1));
        // attempts: 0 → wait 2^0 = 1; 1 → 2^1 = 2; 2 → 2^2 = 4.
        assert_eq!(
            sleeps.borrow().as_slice(),
            &[
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
            ]
        );
    }

    #[tokio::test]
    async fn rate_limit_wait_is_clamped_to_max() {
        let attempts = RefCell::new(0u32);
        let sleeps = RefCell::new(Vec::new());
        let _res: Result<u32, TestError> = with_retry_using(
            || {
                let n = *attempts.borrow();
                *attempts.borrow_mut() += 1;
                async move {
                    if n == 0 {
                        Err(TestError::RateLimited(Some(9_999)))
                    } else {
                        Ok(0)
                    }
                }
            },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(
            sleeps.borrow().as_slice(),
            &[Duration::from_secs(MAX_RETRY_WAIT_SECS)]
        );
    }

    #[tokio::test]
    async fn gives_up_after_max_retries() {
        let attempts = RefCell::new(0u32);
        let sleeps = RefCell::new(Vec::new());
        let res: Result<u32, TestError> = with_retry_using(
            || {
                *attempts.borrow_mut() += 1;
                async { Err::<u32, _>(TestError::RateLimited(Some(1))) }
            },
            recording_sleep(&sleeps),
        )
        .await;
        assert_eq!(res, Err(TestError::RateLimited(Some(1))));
        // MAX_RETRIES retries (each preceded by a sleep) plus the original
        // failing call = MAX_RETRIES + 1 invocations of `f`.
        assert_eq!(*attempts.borrow(), MAX_RETRIES + 1);
        assert_eq!(sleeps.borrow().len() as u32, MAX_RETRIES);
    }

    #[test]
    fn parse_retry_after_accepts_seconds() {
        let h = HeaderValue::from_static("30");
        assert_eq!(parse_retry_after(Some(&h)), Some(30));
    }

    #[test]
    fn parse_retry_after_rejects_http_date_and_garbage() {
        let date = HeaderValue::from_static("Wed, 21 Oct 2015 07:28:00 GMT");
        assert_eq!(parse_retry_after(Some(&date)), None);
        let garbage = HeaderValue::from_static("not-a-number");
        assert_eq!(parse_retry_after(Some(&garbage)), None);
    }

    #[test]
    fn parse_retry_after_handles_missing_header() {
        assert_eq!(parse_retry_after(None), None);
    }

    // `check_status` is exercised indirectly by every existing
    // `tests/jira_client.rs` / `tests/freelo_client.rs` integration test
    // once the clients migrate to call it — we don't bother with a
    // direct unit test here.
}

//! Independent limits for authentication HTTP attempts.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn http_request_policies_keep_connection_and_status_limits_independent() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(async {
            for (global, no_response, status, expected) in [
                (3, 0, None, 1),
                (1, 4, None, 2),
                (4, 1, None, 2),
                (3, 0, Some(503), 4),
                (1, 4, Some(503), 2),
                (3, 4, Some(409), 1),
                (3, 4, Some(199), 4),
            ] {
                let calls = Arc::new(AtomicUsize::new(0));
                let captured = calls.clone();
                let fetch: super::super::Fetch = Arc::new(move |_| {
                    captured.fetch_add(1, Ordering::SeqCst);
                    Box::pin(std::future::ready(attempt(status)))
                });
                let request = HttpRequest {
                    method: "GET".into(),
                    url: "http://controlled.invalid".into(),
                    headers: indexmap::IndexMap::default(),
                    body: vec![],
                    signal: None,
                };
                let policy = RequestPolicy {
                    timeout: None,
                    retries: global,
                    no_response_retries: no_response,
                    statuses: RetryStatuses::Authentication,
                };
                assert!(
                    send_with_policy(request, fetch, policy, status_failure)
                        .await
                        .is_err()
                );
                assert_eq!(calls.load(Ordering::SeqCst), expected);
            }
        });
}

/// Produce a controlled status or a no-response failure.
fn attempt(status: Option<u16>) -> Result<HttpResponse, FetchError> {
    status.map_or(Err(FetchError::Timeout), |status| {
        Ok(HttpResponse {
            status,
            status_text: String::new(),
            headers: BTreeMap::new(),
            body: Box::pin(futures_util::stream::empty()),
        })
    })
}

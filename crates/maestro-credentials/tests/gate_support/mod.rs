//! Provider whose refresh waits on named flags, shared by tests that interleave requests.
#![allow(dead_code)] // each test binary uses a different part of these helpers
use crate::oauth_support::{ControlledProvider, FUTURE, credentials};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Provider whose refresh sets `started`, then waits for `release`.
pub fn gated(
    id: &str,
    started: &Arc<AtomicBool>,
    release: &Arc<AtomicBool>,
) -> Arc<ControlledProvider> {
    let (started, release) = (started.clone(), release.clone());
    ControlledProvider::refreshing(
        id,
        Box::new(move |_| {
            let (started, release) = (started.clone(), release.clone());
            Box::pin(async move {
                started.store(true, Ordering::SeqCst);
                while !release.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
                Ok(credentials("new", FUTURE))
            })
        }),
    )
    .register()
}

//! Caller-owned local executors for native test futures.

#![cfg(not(target_arch = "wasm32"))]

use std::future::Future;

use tokio::runtime::Builder;
use tokio::task::LocalSet;

/// Run one native test future on an executor this call owns.
///
/// # Panics
/// Panics when the runtime cannot build its drivers.
pub fn run(action: impl Future<Output = ()>) {
    let built = Builder::new_current_thread().enable_all().build();
    assert!(built.is_ok(), "the local test runtime builds its drivers");
    if let Ok(runtime) = built {
        LocalSet::new().block_on(&runtime, action);
    }
}

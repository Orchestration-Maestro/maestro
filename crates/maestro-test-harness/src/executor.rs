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
    let completed = block_on(action);
    assert!(
        completed.is_ok(),
        "the local test runtime builds its drivers: {:?}",
        completed.err()
    );
}

/// Run one native test future to its output on an executor this call owns, where local
/// tasks it spawns make progress.
///
/// # Errors
/// Returns the failure building the runtime's drivers.
pub fn block_on<T>(action: impl Future<Output = T>) -> std::io::Result<T> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    Ok(LocalSet::new().block_on(&runtime, action))
}

//! Shared async test driver.

use std::future::Future;

/// Drives supplied futures without test deadlines.
pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

thread_local! {
    /// Resolution and reorder visits on the current test's thread.
    pub(crate) static WORKSPACE_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts a workspace resolution or reorder visit without affecting production builds.
pub(crate) fn record_workspace_work() {
    WORKSPACE_WORK.set(WORKSPACE_WORK.get() + 1);
}

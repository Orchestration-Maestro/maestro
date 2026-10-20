//! Shared async test driver.

use std::future::Future;

/// Drives supplied futures without test deadlines.
pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

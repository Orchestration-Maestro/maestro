//! Test-owned Tokio runtimes shared by the library and integration tests.
#![cfg(test)]

/// Build one current-thread runtime with its I/O and time drivers, its clock paused when asked.
pub fn runtime(paused: bool) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build()
        .unwrap()
}

/// Run `future` to completion on a fresh current-thread runtime.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    runtime(false).block_on(future)
}

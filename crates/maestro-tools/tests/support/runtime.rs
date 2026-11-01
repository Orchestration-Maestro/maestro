//! Test-owned Tokio runtimes shared by the library and integration tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(false).block_on(future)
}

/// Complete `future` on a runtime whose clock starts paused.
pub fn run_paused<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(true).block_on(future)
}

/// Build one current-thread runtime, with its clock paused when asked.
fn runtime(paused: bool) -> tokio::runtime::Runtime {
    builder().start_paused(paused).build().unwrap()
}

/// A current-thread runtime builder for tests that control blocking-pool execution.
pub fn builder() -> tokio::runtime::Builder {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    builder
}

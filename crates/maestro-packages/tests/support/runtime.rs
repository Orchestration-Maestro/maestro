//! Test-owned Tokio runtimes shared by the library and integration tests.
#![cfg(test)]

/// Build one current-thread runtime with its I/O and time drivers.
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Complete `future` on a fresh runtime.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime().block_on(future)
}

//! Test-owned Tokio runtime shared by the library and integration tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
pub fn run_io<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

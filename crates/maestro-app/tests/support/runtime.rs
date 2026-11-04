//! Test-owned Tokio runtimes shared by the library, example and integration tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
pub fn run_io<T>(future: impl std::future::Future<Output = T>) -> T {
    io_runtime().block_on(future)
}

/// Build one current-thread runtime with its I/O and time drivers.
pub fn io_runtime() -> tokio::runtime::Runtime {
    io_builder().build().unwrap()
}

/// Start a current-thread runtime builder with its I/O and time drivers.
pub fn io_builder() -> tokio::runtime::Builder {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    builder
}

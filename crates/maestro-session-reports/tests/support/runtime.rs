//! Test-owned Tokio runtime the report cases run on.
#![cfg(test)]

/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
pub fn run_io<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    builder.build().unwrap().block_on(future)
}

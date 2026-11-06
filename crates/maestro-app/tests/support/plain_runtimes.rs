//! Bare current-thread runtimes: a future with no drivers, and an I/O runtime whose clock starts
//! paused.
#![cfg(not(target_arch = "wasm32"))]

#[cfg(unix)]
use super::runtime::io_builder;

/// Complete `future` on a fresh current-thread runtime without I/O or time drivers.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Build one current-thread runtime with its I/O and time drivers and its clock paused.
#[cfg(unix)]
pub fn paused_io_runtime() -> tokio::runtime::Runtime {
    io_builder().start_paused(true).build().unwrap()
}

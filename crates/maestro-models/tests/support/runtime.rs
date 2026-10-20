//! Test-owned Tokio runtimes shared by the library and integration tests.
#![cfg(test)]

/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(false).block_on(future)
}

/// Complete `future` on a runtime whose clock advances only while every task waits.
pub fn run_paused<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(true).block_on(future)
}

/// Complete `future` on a real-time runtime that pauses its clock in the first poll.
///
/// The device-login witnesses step their waits from that instant, not from the runtime's start.
pub fn run_paused_after_start<T>(future: impl std::future::Future<Output = T>) -> T {
    run(async {
        tokio::time::pause();
        future.await
    })
}

/// Build one current-thread runtime, with its clock paused when asked.
fn runtime(paused: bool) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build()
        .unwrap()
}

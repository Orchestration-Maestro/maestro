//! Test-owned Tokio runtimes shared by the library and integration tests.
#![cfg(not(target_arch = "wasm32"))]
/// Complete `future` on a fresh current-thread runtime with its I/O and time drivers.
///
/// # Panics
/// Panics when the runtime cannot build its drivers.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(false).block_on(future)
}

/// Complete `future` on a runtime whose clock starts paused.
///
/// # Panics
/// Panics when the runtime cannot build its drivers.
pub fn run_paused<T>(future: impl std::future::Future<Output = T>) -> T {
    runtime(true).block_on(future)
}

/// Complete `future` on a real-time runtime that pauses its clock in the first poll.
///
/// The device-login witnesses step their waits from that instant, not from the runtime's start.
///
/// # Panics
/// Panics when the runtime cannot build its drivers.
pub fn run_paused_after_start<T>(future: impl std::future::Future<Output = T>) -> T {
    run(async {
        tokio::time::pause();
        future.await
    })
}

/// Build one current-thread runtime, with its clock paused when asked.
fn runtime(paused: bool) -> tokio::runtime::Runtime {
    let built = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build();
    assert!(
        built.is_ok(),
        "a test runtime builds its drivers: {:?}",
        built.as_ref().err()
    );
    let Ok(runtime) = built else {
        unreachable!("the assertion admits only a built runtime")
    };
    runtime
}

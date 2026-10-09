//! Controlled witnesses for internal response conversion and reduction.

mod events;
mod requests;
mod responses;

/// Fallible test body result.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Drive a future on a fresh controlled runtime.
fn block_on<T>(
    paused: bool,
    future: impl std::future::Future<Output = TestResult<T>>,
) -> TestResult<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build()?
        .block_on(future)
}

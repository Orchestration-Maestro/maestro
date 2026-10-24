//! A delay that a cancellation signal can cut short.
use maestro_models::Cancellation;
use std::time::Duration;

/// The delay was cancelled before it elapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepError;

impl std::fmt::Display for SleepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Aborted")
    }
}

impl std::error::Error for SleepError {}

/// Waits for `duration`, or fails as soon as `signal` is aborted.
///
/// A signal aborted before the call fails without arming a timer.
///
/// # Errors
/// Returns [`SleepError`] when the signal aborts first.
pub async fn sleep(duration: Duration, signal: Option<&Cancellation>) -> Result<(), SleepError> {
    let Some(signal) = signal else {
        tokio::time::sleep(duration).await;
        return Ok(());
    };
    if signal.is_aborted() {
        return Err(SleepError);
    }
    tokio::select! {
        () = tokio::time::sleep(duration) => Ok(()),
        () = signal.cancelled() => Err(SleepError),
    }
}

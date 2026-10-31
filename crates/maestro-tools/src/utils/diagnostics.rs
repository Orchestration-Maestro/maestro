//! Failure records and cancellable native work shared by the tools.

use crate::file_mutation_queue::running_admission;
use futures_util::future::{Either, select};
use maestro_models::{Cancellation, DiagnosticCode, DiagnosticErrorInfo};
use std::{future::Future, io, pin::pin};

/// A failure whose message is `message`, named `Error` with no stack or code.
pub(crate) fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(message)
}

/// Describe a native failure with its message and operating-system error number.
pub(crate) fn diagnostic(error: &io::Error) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        code: error
            .raw_os_error()
            .map(|errno| DiagnosticCode::Number(f64::from(errno))),
        ..failure(&error.to_string())
    }
}

/// Run blocking filesystem work off the caller's task.
///
/// Work started inside a queued file mutation keeps that file's queue held until it returns, so
/// dropping the awaiting future never lets a later mutation overlap it.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, DiagnosticErrorInfo> {
    let admission = running_admission();
    let task = tokio::task::spawn_blocking(move || {
        let result = work();
        drop(admission);
        result
    });
    let result = task.await.map_err(io::Error::other).and_then(|done| done);
    result.map_err(|error| diagnostic(&error))
}

/// The failure every cancelled tool call settles with.
pub(crate) fn aborted() -> DiagnosticErrorInfo {
    failure("Operation aborted")
}

/// Start one operation unless cancelled, and settle as cancelled if the signal fires first.
///
/// Cancellation wins over any result or failure observed after it.
pub(crate) async fn guarded<T, F: Future<Output = Result<T, DiagnosticErrorInfo>>>(
    signal: Option<&Cancellation>,
    start: impl FnOnce() -> F,
) -> Result<T, DiagnosticErrorInfo> {
    let Some(signal) = signal else {
        return start().await;
    };
    let cancelled = signal.cancelled();
    if signal.is_aborted() {
        return Err(aborted());
    }
    match select(pin!(start()), pin!(cancelled)).await {
        Either::Left((result, _)) if !signal.is_aborted() => result,
        _ => Err(aborted()),
    }
}

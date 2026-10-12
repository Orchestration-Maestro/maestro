//! Local-filesystem read operations.

use super::{ReadOperations, failure};
use crate::utils::mime::detect_supported_image_mime_type_from_file;
use maestro_models::{BoxFuture, DiagnosticCode, DiagnosticErrorInfo};
use std::{io, path::PathBuf};

/// Describe a native failure with its message and operating-system error number.
pub(super) fn diagnostic(error: &io::Error) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        code: error
            .raw_os_error()
            .map(|errno| DiagnosticCode::Number(f64::from(errno))),
        ..failure(&error.to_string())
    }
}

/// Run blocking filesystem work off the caller's task.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, DiagnosticErrorInfo> {
    let task = tokio::task::spawn_blocking(work);
    let result = task.await.map_err(io::Error::other).and_then(|done| done);
    result.map_err(|error| diagnostic(&error))
}

/// Check readability without opening the file where the platform allows it.
fn readable(path: &PathBuf) -> io::Result<()> {
    #[cfg(unix)]
    return Ok(rustix::fs::access(path, rustix::fs::Access::READ_OK)?);
    #[cfg(not(unix))]
    return std::fs::File::open(path).map(drop);
}

/// The default operations: read, probe and sniff the local filesystem.
pub(super) struct NativeReadOperations;

impl ReadOperations for NativeReadOperations {
    fn read_file(&self, absolute_path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        Box::pin(async move {
            tokio::fs::read(absolute_path)
                .await
                .map_err(|e| diagnostic(&e))
        })
    }

    fn access(&self, absolute_path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(blocking(move || readable(&PathBuf::from(absolute_path))))
    }

    fn detect_image_mime_type(
        &self,
        absolute_path: String,
    ) -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> {
        Box::pin(blocking(move || {
            detect_supported_image_mime_type_from_file(&absolute_path).map(|m| m.map(str::to_owned))
        }))
    }
}

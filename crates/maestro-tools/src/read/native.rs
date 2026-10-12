//! Local-filesystem read operations.

use super::ReadOperations;
use crate::utils::{
    diagnostics::{blocking, diagnostic},
    mime::detect_supported_image_mime_type_from_file,
};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::{io, path::PathBuf};

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

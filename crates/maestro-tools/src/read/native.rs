//! Local-filesystem read operations.

use super::ReadOperations;
use crate::utils::{
    diagnostics::{blocking, diagnostic},
    mime::detect_supported_image_mime_type_from_file,
};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::{io, path::PathBuf};

/// Check read access and optionally write access without creating or truncating a file.
pub(crate) fn access_file(
    absolute_path: String,
    write: bool,
) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
    Box::pin(blocking(move || {
        probe(&PathBuf::from(absolute_path), write)
    }))
}

/// Probe the caller-selected access mode.
fn probe(path: &PathBuf, write: bool) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mode = if write {
            rustix::fs::Access::READ_OK | rustix::fs::Access::WRITE_OK
        } else {
            rustix::fs::Access::READ_OK
        };
        Ok(rustix::fs::access(path, mode)?)
    }
    // As access(2) behaves on Windows: the entry exists, and a write needs a file that is
    // not read-only. A directory passes, so reading it reports the real failure.
    #[cfg(not(unix))]
    {
        let metadata = std::fs::metadata(path)?;
        if write && metadata.permissions().readonly() && !metadata.is_dir() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
}

/// The default operations: read, probe and sniff the local filesystem.
pub(crate) struct NativeReadOperations;

impl ReadOperations for NativeReadOperations {
    fn read_file(&self, absolute_path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        Box::pin(async move {
            tokio::fs::read(absolute_path)
                .await
                .map_err(|e| diagnostic(&e))
        })
    }

    fn access(&self, absolute_path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        access_file(absolute_path, false)
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

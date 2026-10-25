//! Read-only edit previews over the existing read operations.

use super::{
    Edit, EditDiffResult, apply_edits_to_normalized_content, generate_diff_string, normalize_to_lf,
    strip_bom,
};
use crate::{
    path_utils::resolve_existing_path,
    read::{NativeReadOperations, ReadOperations},
};
use maestro_models::{BoxFuture, DiagnosticCode, DiagnosticErrorInfo};
use std::io;

/// A caught access, read or matching failure during a preview.
#[derive(Debug, PartialEq, Eq)]
pub struct EditDiffError {
    /// The failure text.
    pub error: String,
}

/// Calculate a read-only preview of edits against one original snapshot.
///
/// Path resolution happens when called; resolution errors are returned before a future is created.
/// The preview checks readability, reads the target and returns a diff or caught failure without writing it.
///
/// # Errors
/// Returns path-resolution failures directly; access, read and matching failures are in the future's result.
pub fn compute_edits_diff(
    path: &str,
    edits: Vec<Edit>,
    cwd: &str,
) -> io::Result<BoxFuture<Result<EditDiffResult, EditDiffError>>> {
    let absolute = resolve_existing_path(path, cwd)?;
    let path = path.to_owned();
    Ok(Box::pin(async move {
        resolved_preview(&path, absolute, &edits, &NativeReadOperations).await
    }))
}

/// Calculate a read-only preview of one replacement.
///
/// Path resolution happens when called; resolution errors are returned before a future is created.
///
/// # Errors
/// Returns path-resolution failures directly; access, read and matching failures are in the future's result.
pub fn compute_edit_diff(
    path: &str,
    old_text: &str,
    new_text: &str,
    cwd: &str,
) -> io::Result<BoxFuture<Result<EditDiffResult, EditDiffError>>> {
    compute_edits_diff(
        path,
        vec![Edit {
            old_text: old_text.to_owned(),
            new_text: new_text.to_owned(),
        }],
        cwd,
    )
}

/// Keep supplied codes intact while adding the authored path to an access failure.
pub(crate) fn access_failure(path: &str, failure: DiagnosticErrorInfo) -> EditDiffError {
    let detail = match failure.code {
        Some(DiagnosticCode::Text(code)) => format!("Error code: {code}"),
        Some(DiagnosticCode::Number(code)) => {
            format!("Error code: {}", ryu_js::Buffer::new().format(code))
        }
        None => failure.message,
    };
    EditDiffError {
        error: format!("Could not edit file: {path}. {detail}."),
    }
}

/// Read and match a previously resolved target, preserving the two caught failure phases.
async fn resolved_preview(
    path: &str,
    absolute: String,
    edits: &[Edit],
    operations: &dyn ReadOperations,
) -> Result<EditDiffResult, EditDiffError> {
    operations
        .access(absolute.clone())
        .await
        .map_err(|failure| access_failure(path, failure))?;
    let bytes = operations
        .read_file(absolute)
        .await
        .map_err(|failure| EditDiffError {
            error: failure.message,
        })?;
    let text = String::from_utf8_lossy(&bytes);
    let (_, content) = strip_bom(&text);
    let applied = apply_edits_to_normalized_content(&normalize_to_lf(content), edits, path)
        .map_err(|failure| EditDiffError {
            error: failure.message,
        })?;
    Ok(generate_diff_string(
        &applied.base_content,
        &applied.new_content,
        None,
    ))
}

#[cfg(test)]
mod tests;

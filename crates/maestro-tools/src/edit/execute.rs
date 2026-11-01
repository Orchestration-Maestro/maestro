//! Ordered edit effects composed from the matching and native I/O owners.

use super::{EditOperations, EditToolDetails, EditToolInput};
use crate::{
    edit_diff::{
        access_failure, apply_edits_to_normalized_content, detect_line_ending,
        generate_diff_string, normalize_to_lf, restore_line_endings, strip_bom,
    },
    read::{NativeReadOperations, ReadOperations, access_file},
    utils::diagnostics::{check_aborted, failure},
    write::{NativeWriteOperations, WriteOperations},
};
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use std::sync::Arc;

/// Local operations delegating to the existing read and write adapters.
pub(super) struct NativeEditOperations;
impl EditOperations for NativeEditOperations {
    fn read_file(&self, path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        NativeReadOperations.read_file(path)
    }
    fn write_file(
        &self,
        path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        NativeWriteOperations.write_file(path, content)
    }
    fn access(&self, path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        access_file(path, true)
    }
}

/// Access, read, apply and write an admitted edit, then return its LF diff.
pub(super) async fn edit_file(
    operations: Arc<dyn EditOperations>,
    absolute: String,
    input: EditToolInput,
    signal: Option<Cancellation>,
) -> Result<AgentToolResult<EditToolDetails>, DiagnosticErrorInfo> {
    let signal = signal.as_ref();
    check_aborted(signal)?;
    operations
        .access(absolute.clone())
        .await
        .map_err(|error| failure(&access_failure(&input.path, error).error))?;
    check_aborted(signal)?;
    let bytes = operations.read_file(absolute.clone()).await?;
    check_aborted(signal)?;
    let text = String::from_utf8_lossy(&bytes);
    let (bom, content) = strip_bom(&text);
    let ending = detect_line_ending(content);
    let applied =
        apply_edits_to_normalized_content(&normalize_to_lf(content), &input.edits, &input.path)?;
    let restored = format!(
        "{bom}{}",
        restore_line_endings(&applied.new_content, ending)
    );
    check_aborted(signal)?;
    operations.write_file(absolute, restored).await?;
    check_aborted(signal)?;
    Ok(AgentToolResult::text_result(
        format!(
            "Successfully replaced {} block(s) in {}.",
            input.edits.len(),
            input.path
        ),
        Some(generate_diff_string(
            &applied.base_content,
            &applied.new_content,
            None,
        )),
    ))
}

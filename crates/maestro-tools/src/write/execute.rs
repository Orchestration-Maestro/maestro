//! Ordered, cancellable execution of one write.

use super::{WriteOperations, WriteToolInput};
use crate::utils::diagnostics::{blocking, guarded};
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use std::sync::Arc;

/// The default operations: write the local filesystem.
pub(crate) struct NativeWriteOperations;

impl WriteOperations for NativeWriteOperations {
    fn write_file(
        &self,
        absolute_path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(blocking(move || std::fs::write(absolute_path, content)))
    }

    fn mkdir(&self, dir: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(blocking(move || std::fs::create_dir_all(dir)))
    }
}

/// Create the parent directories of `paths.0`, whose parent is `paths.1`, then replace its
/// content with `input.content`, honouring `signal` before and after each operation.
///
/// The result reports the UTF-8 bytes written and the path as the model authored it.
pub(super) async fn write_file(
    operations: Arc<dyn WriteOperations>,
    paths: (String, String),
    input: WriteToolInput,
    signal: Option<Cancellation>,
) -> Result<AgentToolResult<()>, DiagnosticErrorInfo> {
    let (absolute_path, dir) = paths;
    let signal = signal.as_ref();
    guarded(signal, || operations.mkdir(dir)).await?;
    let bytes = input.content.len();
    guarded(signal, || {
        operations.write_file(absolute_path, input.content)
    })
    .await?;
    Ok(AgentToolResult::text_result(
        format!("Successfully wrote {bytes} bytes to {}", input.path),
        None,
    ))
}

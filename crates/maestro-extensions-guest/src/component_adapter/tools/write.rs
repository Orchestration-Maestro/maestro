//! Reverse write effects preserve original content and structured diagnostics.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, WriteOperation};

/// Runs one retained effect after leaving the callback-table borrow.
pub(crate) async fn invoke(
    handler: u32,
    operation: WriteOperation,
    path: String,
) -> Result<(), DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::WriteOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            WriteOperation::Mkdir => (operations.mkdir)(path).await,
            WriteOperation::WriteFile(content) => (operations.write_file)(path, content).await,
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

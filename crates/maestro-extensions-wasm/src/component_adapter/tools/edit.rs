//! Reverse edit effects keep bytes and diagnostic fields intact for the native consumer.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, EditOperation, ReadOutput};

/// Runs one original edit effect after leaving the callback-table borrow.
pub(crate) async fn invoke(
    handler: u32,
    operation: EditOperation,
    path: String,
) -> Result<ReadOutput, DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::EditOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            EditOperation::ReadFile => (operations.read_file)(path).await.map(ReadOutput::Bytes),
            EditOperation::WriteFile(content) => (operations.write_file)(path, content)
                .await
                .map(|()| ReadOutput::Unit),
            EditOperation::Access => (operations.access)(path).await.map(|()| ReadOutput::Unit),
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

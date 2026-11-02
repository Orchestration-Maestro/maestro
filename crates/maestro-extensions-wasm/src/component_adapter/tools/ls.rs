//! Reverse directory observations preserve original names and diagnostics.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, LsOperation, LsOutput};

/// Runs an observation only after releasing the callback-table borrow.
pub(crate) async fn invoke(
    handler: u32,
    operation: LsOperation,
    path: String,
) -> Result<LsOutput, DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::LsOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            LsOperation::Exists => (operations.exists)(path).await.map(LsOutput::Boolean),
            LsOperation::Stat => (operations.stat)(path).await.map(LsOutput::Boolean),
            LsOperation::Readdir => (operations.readdir)(path).await.map(LsOutput::Names),
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

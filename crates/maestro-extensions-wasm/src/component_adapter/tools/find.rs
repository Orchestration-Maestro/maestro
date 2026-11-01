//! Reverse find observations retain original operands, order and diagnostics.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, FindOperation, FindOutput};

/// Runs one observation after leaving the callback-table borrow.
pub(crate) async fn invoke(
    handler: u32,
    operation: FindOperation,
    path: String,
) -> Result<FindOutput, DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::FindOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            FindOperation::Exists => (operations.exists)(path).await.map(FindOutput::Exists),
            FindOperation::Glob(glob) => (operations.glob)(
                glob.pattern,
                path,
                crate::FindGlobOptions {
                    ignore: glob.options.ignore,
                    limit: glob.options.limit,
                },
            )
            .await
            .map(FindOutput::Paths),
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

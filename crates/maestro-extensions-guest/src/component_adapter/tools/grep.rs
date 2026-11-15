//! Reverse content search observations leave searching and output selection native-owned.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, GrepOperation, GrepOutput};

/// Runs an original observation after releasing the callback-table borrow.
pub(crate) async fn invoke(
    handler: u32,
    operation: GrepOperation,
    path: String,
) -> Result<GrepOutput, DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::GrepOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            GrepOperation::IsDirectory => (operations.is_directory)(path)
                .await
                .map(GrepOutput::Directory),
            GrepOperation::ReadFile => (operations.read_file)(path).await.map(GrepOutput::Text),
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

//! Reverse read operations run the retained author callbacks without converting bytes.
use super::super::callbacks;
use crate::bindings::maestro::extension::host::{DiagnosticErrorInfo, ReadOperation, ReadOutput};

/// Executes one original observation, retaining its owner until the future settles.
pub(crate) async fn invoke(
    handler: u32,
    operation: ReadOperation,
    path: String,
) -> Result<ReadOutput, DiagnosticErrorInfo> {
    let result = async {
        let operations = callbacks::find::<crate::ReadOperations>(handler)
            .map_err(crate::DiagnosticErrorInfo::error)?;
        match operation {
            ReadOperation::ReadFile => (operations.read_file)(path).await.map(ReadOutput::Bytes),
            ReadOperation::Access => (operations.access)(path).await.map(|()| ReadOutput::Unit),
            ReadOperation::DetectImageMimeType => match operations.detect_image_mime_type {
                Some(detect) => detect(path).await.map(ReadOutput::Mime),
                None => Ok(ReadOutput::Mime(None)),
            },
        }
    }
    .await;
    result.map_err(crate::schema_error::to_wire)
}

//! Attachment classification.
use crate::{Attachment, browser::attachment_content::FileType};
use maestro_app::browser::utils::attachment_utils::AttachmentType;
/// Select the first matching document presentation.
pub(super) fn file_type(a: &Attachment) -> FileType {
    if a.r#type == AttachmentType::Image {
        FileType::Image
    } else if a.mime_type == "application/pdf" {
        FileType::Pdf
    } else if a.mime_type.contains("wordprocessingml") {
        FileType::Docx
    } else if a.mime_type.contains("presentationml")
        || a.file_name.to_ascii_lowercase().ends_with(".pptx")
    {
        FileType::Pptx
    } else if a.mime_type.contains("spreadsheetml")
        || a.mime_type.contains("ms-excel")
        || [".xlsx", ".xls"]
            .iter()
            .any(|s| a.file_name.to_ascii_lowercase().ends_with(s))
    {
        FileType::Excel
    } else {
        FileType::Text
    }
}

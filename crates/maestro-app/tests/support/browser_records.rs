//! Strict attachment operands for browser message conversion.
#![cfg(test)]
use maestro_app::browser::utils::attachment_utils::{Attachment, AttachmentType};
use serde::Deserialize;
use serde_json::Value;
/// An attachment input from the corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AttachmentRecord {
    /// Identifier.
    id: String,
    /// Classification.
    r#type: String,
    /// Authored name.
    file_name: String,
    /// Media type.
    mime_type: String,
    /// Byte length.
    size: usize,
    /// Original bytes.
    content: String,
    /// Derived text.
    extracted_text: Option<String>,
    /// Derived image.
    preview: Option<String>,
}
impl AttachmentRecord {
    /// Construct the application-owned value using every recorded field.
    pub fn into_attachment(self) -> Attachment {
        Attachment {
            id: self.id,
            r#type: match self.r#type.as_str() {
                "image" => AttachmentType::Image,
                "document" => AttachmentType::Document,
                _ => panic!("unexpected attachment classification"),
            },
            file_name: self.file_name,
            mime_type: self.mime_type,
            size: self.size,
            content: self.content,
            extracted_text: self.extracted_text,
            preview: self.preview,
        }
    }
}
/// Construct all attachment inputs.
pub fn attachments(value: Value) -> Vec<Attachment> {
    serde_json::from_value::<Vec<AttachmentRecord>>(value)
        .unwrap()
        .into_iter()
        .map(AttachmentRecord::into_attachment)
        .collect()
}

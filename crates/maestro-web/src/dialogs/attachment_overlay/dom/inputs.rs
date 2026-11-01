//! Selected attachment operands used to retain unchanged document bodies.
use super::super::content::file_type;
use crate::{Attachment, browser::attachment_content::FileType};

/// Last published file operands, not a replacement for the retained application handle.
pub(super) struct Inputs {
    /// Selected format.
    pub kind: FileType,
    /// Literal original filename.
    pub name: String,
    /// Original media type.
    pub mime: String,
    /// Original encoded bytes.
    pub content: String,
    /// Text only when the selected branch displays it.
    pub text: Option<String>,
}
impl Inputs {
    /// Capture only operands needed after releasing the original record's guard.
    pub fn read(attachment: &Attachment, extracted: bool) -> Self {
        Self {
            kind: file_type(attachment),
            name: attachment.file_name.clone(),
            mime: attachment.mime_type.clone(),
            content: attachment.content.clone(),
            text: selected_text(attachment, extracted).map(str::to_owned),
        }
    }
    /// Compare before copying bytes or resetting a live document.
    pub fn matches(&self, attachment: &Attachment, extracted: bool) -> bool {
        self.kind == file_type(attachment)
            && self.name == attachment.file_name
            && self.mime == attachment.mime_type
            && self.content == attachment.content
            && self.text.as_deref() == selected_text(attachment, extracted)
    }
}
/// Empty text selects the localized fallback, while whitespace remains literal.
fn selected_text(attachment: &Attachment, extracted: bool) -> Option<&str> {
    let kind = file_type(attachment);
    (kind != FileType::Image && (extracted || matches!(kind, FileType::Text | FileType::Pptx)))
        .then(|| {
            attachment
                .extracted_text
                .as_deref()
                .filter(|text| !text.is_empty())
        })
        .flatten()
}

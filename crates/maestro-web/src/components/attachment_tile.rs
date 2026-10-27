//! Attachment thumbnails with optional removal.
use crate::SharedAttachment;
#[cfg(target_arch = "wasm32")]
use crate::dialogs::attachment_overlay::AttachmentUiError;
#[cfg(target_arch = "wasm32")]
use maestro_app::{browser::utils::attachment_utils::AttachmentType, truncate_utf16};
use std::rc::Rc;

/// A native attachment thumbnail with optional removal.
pub struct AttachmentTile {
    /// Original mutable application record.
    pub attachment: SharedAttachment,
    /// Show the independent remove control.
    pub show_delete: bool,
    /// Current removal notification.
    pub on_delete: Option<Rc<dyn Fn()>>,
}
impl From<SharedAttachment> for AttachmentTile {
    fn from(attachment: SharedAttachment) -> Self {
        Self {
            attachment,
            show_delete: false,
            on_delete: None,
        }
    }
}
/// Copy only operands needed outside the record borrow.
#[cfg(target_arch = "wasm32")]
fn thumbnail_data(
    attachment: &crate::SharedAttachment,
) -> Result<(String, Option<String>, bool, bool), AttachmentUiError> {
    let a = attachment
        .try_read()
        .map_err(|_| AttachmentUiError::BorrowConflict)?;
    let mime = if a.r#type == AttachmentType::Image {
        a.mime_type.as_str()
    } else {
        "image/png"
    };
    Ok((
        a.file_name.clone(),
        a.preview
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("data:{mime};base64,{s}")),
        a.mime_type == "application/pdf",
        a.mime_type.contains("spreadsheetml")
            || [".xlsx", ".xls"]
                .iter()
                .any(|s| a.file_name.to_ascii_lowercase().ends_with(s)),
    ))
}

/// Keep whole characters within the shortened filename prefix.
#[cfg(target_arch = "wasm32")]
fn filename_label(name: &str) -> String {
    if name.encode_utf16().count() > 10 {
        format!("{}...", truncate_utf16(name, 8))
    } else {
        name.to_owned()
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;

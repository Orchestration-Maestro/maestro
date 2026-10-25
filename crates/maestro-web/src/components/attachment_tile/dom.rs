//! Attachment thumbnails mounted with original shared records and native controls.
use crate::SharedAttachment;
use std::rc::Rc;

/// A native attachment thumbnail with optional removal.
#[doc(hidden)]
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
#[cfg(target_arch = "wasm32")]
mod browser;

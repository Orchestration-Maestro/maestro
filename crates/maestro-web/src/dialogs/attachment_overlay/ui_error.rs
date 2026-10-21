//! Errors owned by this operation interface.
use crate::{
    I18nError,
    browser::attachment_content::{AttachmentHostError, ControlError},
};
/// Failure from the retained attachment operation.
#[derive(Debug)]
pub enum AttachmentUiError {
    /// Localization failed.
    I18n(I18nError),
    /// Shared control drawing failed.
    Control(ControlError),
    /// Native operation failed.
    Host(AttachmentHostError),
    /// Selected presentation could not be prepared.
    Presentation(String),
    /// Caller holds an incompatible record borrow.
    BorrowConflict,
}
crate::ui_error::impl_ui_error!(AttachmentUiError, _ => None);
impl From<I18nError> for AttachmentUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<ControlError> for AttachmentUiError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}
impl From<AttachmentHostError> for AttachmentUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}

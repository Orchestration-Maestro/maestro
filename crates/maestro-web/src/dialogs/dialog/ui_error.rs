//! Errors owned by this operation interface.
use crate::{
    I18nError,
    browser::attachment_content::{AttachmentHostError, ControlError},
};
/// Failure from a retained dialog or attachment overlay operation.
#[derive(Debug)]
pub enum DialogUiError {
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
impl std::fmt::Display for DialogUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I18n(error) => std::fmt::Display::fmt(error, f),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
            Self::Host(error) => std::fmt::Display::fmt(error, f),
            Self::Presentation(error) => std::fmt::Display::fmt(error, f),
            Self::BorrowConflict => f.write_str("Attachment is already borrowed"),
        }
    }
}
impl std::error::Error for DialogUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I18n(error) => Some(error),
            Self::Control(error) => std::error::Error::source(error),
            Self::Host(error) => Some(error),
            _ => None,
        }
    }
}
impl From<I18nError> for DialogUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<ControlError> for DialogUiError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}
impl From<AttachmentHostError> for DialogUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}

//! Errors shared by code and Markdown reading operations.
use crate::{I18nError, browser::attachment_content::AttachmentHostError};
/// Reading preparation or native publication failure.
#[derive(Debug)]
pub enum ReadingUiError {
    /// Outer reading style could not be resolved.
    Presentation(std::io::Error),
    /// Syntax highlighting failed.
    Highlight(Box<dyn std::error::Error + Send + Sync>),
    /// Localization failed.
    I18n(I18nError),
    /// Localized control drawing failed.
    Control(crate::components::copy_button::LocalizedControlError),
    /// Native operation failed.
    Host(AttachmentHostError),
}
impl std::fmt::Display for ReadingUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Presentation(error) => std::fmt::Display::fmt(error, f),
            Self::Highlight(error) => std::fmt::Display::fmt(error, f),
            Self::I18n(error) => std::fmt::Display::fmt(error, f),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
            Self::Host(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for ReadingUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Presentation(error) => Some(error),
            Self::Highlight(error) => Some(error.as_ref()),
            Self::I18n(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Host(error) => Some(error),
        }
    }
}
impl From<I18nError> for ReadingUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<crate::components::copy_button::LocalizedControlError> for ReadingUiError {
    fn from(error: crate::components::copy_button::LocalizedControlError) -> Self {
        Self::Control(error)
    }
}
impl From<AttachmentHostError> for ReadingUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}

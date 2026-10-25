//! Errors shared by localized copy and preview controls.
use crate::{I18nError, browser::attachment_content::ControlError};
/// Localized control operation failure.
#[derive(Debug)]
pub enum LocalizedControlError {
    /// Localization failed.
    I18n(I18nError),
    /// Shared control drawing failed.
    Control(ControlError),
    /// Selected control colors could not be resolved.
    Presentation(std::io::Error),
}
impl std::fmt::Display for LocalizedControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I18n(error) => std::fmt::Display::fmt(error, f),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
            Self::Presentation(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for LocalizedControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I18n(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Presentation(error) => Some(error),
        }
    }
}
impl From<I18nError> for LocalizedControlError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<ControlError> for LocalizedControlError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}

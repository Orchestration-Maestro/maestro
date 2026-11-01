//! Errors owned by tool rendering operations.
use crate::{
    I18nError,
    browser::attachment_content::{AttachmentHostError, ControlError},
    components::code_block::ReadingUiError,
};
/// Tool rendering operation failure.
#[derive(Debug)]
pub enum ToolUiError {
    /// Localization failed.
    I18n(I18nError),
    /// Shared control drawing failed.
    Control(ControlError),
    /// Native operation failed.
    Host(AttachmentHostError),
    /// Reading preparation failed.
    Reading(ReadingUiError),
    /// Attachment tile or overlay failed.
    Attachment(crate::dialogs::attachment_overlay::AttachmentUiError),
    /// Tool operands or presentation could not be prepared.
    Presentation(String),
}
impl std::fmt::Display for ToolUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I18n(error) => std::fmt::Display::fmt(error, f),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
            Self::Host(error) => std::fmt::Display::fmt(error, f),
            Self::Reading(error) => std::fmt::Display::fmt(error, f),
            Self::Attachment(error) => std::fmt::Display::fmt(error, f),
            Self::Presentation(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for ToolUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I18n(error) => Some(error),
            Self::Control(error) => std::error::Error::source(error),
            Self::Host(error) => Some(error),
            Self::Reading(error) => Some(error),
            Self::Attachment(error) => Some(error),
            Self::Presentation(_) => None,
        }
    }
}
impl From<I18nError> for ToolUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<ControlError> for ToolUiError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}
impl From<AttachmentHostError> for ToolUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}
impl ToolUiError {
    /// Preserve a native browser failure at the tool presentation boundary.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn native(error: &wasm_bindgen::JsValue) -> Self {
        Self::Host(crate::browser::attachment_content::browser::host_error(
            error,
        ))
    }

    /// Retain native and leaf phases while composing reading preparation causes.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn reading(error: ReadingUiError) -> Self {
        match error {
            ReadingUiError::I18n(error) => Self::I18n(error),
            ReadingUiError::Host(error) => Self::Host(error),
            ReadingUiError::Control(error) => match error {
                crate::components::copy_button::LocalizedControlError::I18n(error) => {
                    Self::I18n(error)
                }
                crate::components::copy_button::LocalizedControlError::Control(error) => {
                    Self::Control(error)
                }
                error @ crate::components::copy_button::LocalizedControlError::Presentation(_) => {
                    Self::Reading(ReadingUiError::Control(error))
                }
            },
            error @ (ReadingUiError::Highlight(_) | ReadingUiError::Presentation(_)) => {
                Self::Reading(error)
            }
        }
    }
}

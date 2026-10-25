//! Errors owned by this operation interface.
use super::presentation_cause::PresentationCause;
use crate::{
    I18nError,
    browser::attachment_content::{AttachmentHostError, ControlError},
};
/// Failure from the retained artifact operation.
#[derive(Debug)]
pub enum ArtifactUiError {
    /// Localization failed.
    I18n(I18nError),
    /// Shared control drawing failed.
    Control(ControlError),
    /// Native operation failed.
    Host(AttachmentHostError),
    /// Artifact presentation preparation failed.
    Presentation(PresentationCause),
    /// Caller holds an incompatible record borrow.
    BorrowConflict,
}
crate::ui_error::impl_ui_error!(ArtifactUiError, error => std::error::Error::source(error));
impl From<I18nError> for ArtifactUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<ControlError> for ArtifactUiError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}
impl From<AttachmentHostError> for ArtifactUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}
impl From<ArtifactUiError> for crate::tools::types::ToolUiError {
    fn from(error: ArtifactUiError) -> Self {
        match error {
            ArtifactUiError::I18n(error) => Self::I18n(error),
            ArtifactUiError::Control(error) => Self::Control(error),
            ArtifactUiError::Host(error) => Self::Host(error),
            ArtifactUiError::Presentation(error) => Self::Presentation(error.to_string()),
            error @ ArtifactUiError::BorrowConflict => Self::Presentation(error.to_string()),
        }
    }
}

impl ArtifactUiError {
    /// Preserve native and leaf phases while retaining typed reading preparation causes.
    pub(super) fn reading(error: crate::components::code_block::ReadingUiError) -> Self {
        use crate::components::code_block::ReadingUiError;
        match error {
            error @ (ReadingUiError::Highlight(_) | ReadingUiError::Presentation(_)) => {
                Self::Presentation(PresentationCause::Reading(error))
            }
            ReadingUiError::I18n(error) => Self::I18n(error),
            ReadingUiError::Control(error) => Self::localized_control(error),
            ReadingUiError::Host(error) => Self::Host(error),
        }
    }
    /// Keep localization and glyph failures outside the control-color preparation phase.
    pub(super) fn localized_control(
        error: crate::components::copy_button::LocalizedControlError,
    ) -> Self {
        use crate::components::copy_button::LocalizedControlError;
        match error {
            LocalizedControlError::I18n(error) => Self::I18n(error),
            LocalizedControlError::Control(error) => Self::Control(error),
            error @ LocalizedControlError::Presentation(_) => {
                Self::Presentation(PresentationCause::Control(error))
            }
        }
    }
}

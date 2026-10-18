//! Closed preparation causes carried by artifact presentation.
/// Cause of artifact presentation preparation failure.
#[derive(Debug)]
pub enum PresentationCause {
    /// Existing literal presentation diagnostic.
    Message(String),
    /// Reading preparation failed.
    Reading(crate::components::code_block::ReadingUiError),
    /// Localized control preparation failed.
    Control(crate::components::copy_button::LocalizedControlError),
}
impl std::fmt::Display for PresentationCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(error) => std::fmt::Display::fmt(error, f),
            Self::Reading(error) => std::fmt::Display::fmt(error, f),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for PresentationCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reading(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Message(_) => None,
        }
    }
}
impl From<String> for PresentationCause {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

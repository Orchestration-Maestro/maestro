//! Errors from shared glyph and control drawing.
use crate::icons::IconError;
/// Shared control drawing failure.
#[derive(Debug)]
pub enum ControlError {
    /// Glyph rasterization failed.
    Icon(IconError),
    /// Glyph catalog lookup failed.
    Host(String),
    /// Selected control colors could not be resolved.
    Presentation(String),
}
impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Icon(error) => std::fmt::Display::fmt(error, f),
            Self::Host(error) | Self::Presentation(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for ControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Icon(error) => Some(error),
            _ => None,
        }
    }
}
impl From<IconError> for ControlError {
    fn from(error: IconError) -> Self {
        Self::Icon(error)
    }
}

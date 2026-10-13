//! Typed brand admission and projection failures.
use crate::BrandMode;
use std::fmt;

/// A failure to admit or project brand data.
#[derive(Debug)]
pub enum BrandError {
    /// Invalid JSON or record shape.
    Decode(serde_json::Error),
    /// Out-of-range measurement at a JSON Pointer.
    Measurement(String),
    /// The selected mode is absent.
    MissingMode(BrandMode),
    /// A selected alias names an absent palette key.
    MissingPaletteColor(String),
    /// A selected palette value is not a six-digit hexadecimal color.
    InvalidColor(String),
    /// A required font role is absent.
    MissingFont(String),
    /// A required selected color role is absent.
    MissingColorRole(String),
    /// A reached template color token has no closing parenthesis.
    UnterminatedColor,
}

impl fmt::Display for BrandError {
    /// Render the owning failure category and its operand.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "Invalid brand pack: {error}"),
            Self::Measurement(path) => write!(f, "Invalid brand measurement: {path}"),
            Self::MissingMode(mode) => write!(f, "Missing brand mode: {}", mode.name()),
            Self::MissingPaletteColor(key) => write!(f, "Missing brand palette color: {key}"),
            Self::InvalidColor(color) => write!(f, "Invalid brand color: {color}"),
            Self::MissingFont(role) => write!(f, "Missing brand font: {role}"),
            Self::MissingColorRole(role) => write!(f, "Missing brand color role: {role}"),
            Self::UnterminatedColor => f.write_str("Unterminated brand template color"),
        }
    }
}

impl std::error::Error for BrandError {
    /// Expose the stored native decoder error, when present.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

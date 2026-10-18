//! Image command records and parsing.

mod transmission;
pub use transmission::{Compression, Format, Medium, Transmission};
mod display;
pub use display::{CursorMovement, Display};
mod animation;
pub use animation::{
    AnimationAction, AnimationControl, AnimationFrameComposition, AnimationFrameLoading,
    Background, CompositionMode,
};
mod delete;
pub use delete::Delete;
mod response;
pub use response::Response;
/// Response suppression policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Quiet {
    /// Reply to both successful and failed commands.
    #[default]
    No,
    /// Suppress success replies.
    Ok,
    /// Suppress both success and failure replies.
    Failures,
}

/// Selected command parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Query.
    Query(Transmission),
    /// Transmit.
    Transmit(Transmission),
    /// Transmit And Display.
    TransmitAndDisplay {
        /// Transmission.
        transmission: Transmission,
        /// Display.
        display: Display,
    },
    /// Display.
    Display(Display),
    /// Delete.
    Delete(Delete),
    /// Transmit Animation Frame.
    TransmitAnimationFrame(AnimationFrameLoading),
    /// Control Animation.
    ControlAnimation(AnimationControl),
    /// Compose Animation.
    ComposeAnimation(AnimationFrameComposition),
}

/// Parsed image command with owned payload.
#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    /// Control.
    pub control: Control,
    /// Quiet.
    pub quiet: Quiet,
    /// Data.
    pub data: Vec<u8>,
}

mod parser;
pub use parser::Parser;

impl Command {
    /// Move out the payload while preserving the control record.
    #[must_use]
    pub fn to_owned_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.data)
    }
    /// Selected transmission or query parameters, if present.
    #[must_use]
    pub fn transmission(&self) -> Option<&Transmission> {
        match &self.control {
            Control::Query(t)
            | Control::Transmit(t)
            | Control::TransmitAndDisplay {
                transmission: t, ..
            } => Some(t),
            _ => None,
        }
    }
    /// Selected deferred display parameters, if present.
    #[must_use]
    pub fn display(&self) -> Option<&Display> {
        match &self.control {
            Control::Display(d) | Control::TransmitAndDisplay { display: d, .. } => Some(d),
            _ => None,
        }
    }
}
impl Control {
    /// Select the command's typed record before payload decoding.
    fn parse(keys: &parser::Keys) -> Result<Self, ParseError> {
        let action =
            u8::try_from(keys.get(b'a', u32::from(b't'))).map_err(|_| ParseError::InvalidFormat)?;
        Ok(match action {
            b'q' => Self::Query(Transmission::parse(keys)?),
            b't' => Self::Transmit(Transmission::parse(keys)?),
            b'p' => Self::Display(Display::parse(keys)?),
            b'd' => Self::Delete(Delete::parse(keys)?),
            b'f' => Self::TransmitAnimationFrame(AnimationFrameLoading::parse(keys)?),
            b'a' => Self::ControlAnimation(AnimationControl::parse(keys)?),
            b'c' => Self::ComposeAnimation(AnimationFrameComposition::parse(keys)?),
            b'T' => Self::TransmitAndDisplay {
                transmission: Transmission::parse(keys)?,
                display: Display::parse(keys)?,
            },
            _ => return Err(ParseError::InvalidFormat),
        })
    }
}

/// Image command parsing failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Incomplete controls or an unsupported control discriminant.
    InvalidFormat,
    /// Malformed payload encoding.
    InvalidData,
    /// Invalid decimal input.
    InvalidCharacter,
    /// Decimal input exceeds the selected 32-bit domain.
    Overflow,
    /// The encoded payload exceeds its budget or allocation failed.
    OutOfMemory,
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ParseError {}

//! Parsed animation records.

use super::ParseError;
/// Animation frame loading parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationFrameLoading {
    /// X.
    pub x: u32,
    /// Y.
    pub y: u32,
    /// Create frame.
    pub create_frame: u32,
    /// Edit frame.
    pub edit_frame: u32,
    /// Gap ms.
    pub gap_ms: u32,
    /// Composition mode.
    pub composition_mode: CompositionMode,
    /// Background.
    pub background: Background,
}

/// Animation frame composition parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationFrameComposition {
    /// Frame.
    pub frame: u32,
    /// Edit frame.
    pub edit_frame: u32,
    /// X.
    pub x: u32,
    /// Y.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Left edge.
    pub left_edge: u32,
    /// Top edge.
    pub top_edge: u32,
    /// Composition mode.
    pub composition_mode: CompositionMode,
}

/// Animation playback parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationControl {
    /// Action.
    pub action: AnimationAction,
    /// Frame.
    pub frame: u32,
    /// Gap ms.
    pub gap_ms: u32,
    /// Current frame.
    pub current_frame: u32,
    /// Loops.
    pub loops: u32,
}

/// Background RGBA channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Background {
    /// R.
    pub r: u8,
    /// G.
    pub g: u8,
    /// B.
    pub b: u8,
    /// A.
    pub a: u8,
}

/// Frame compositing operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CompositionMode {
    /// Alpha Blend.
    #[default]
    AlphaBlend,
    /// Overwrite.
    Overwrite,
}

/// Playback operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimationAction {
    /// Invalid.
    #[default]
    Invalid,
    /// Stop.
    Stop,
    /// Run Wait.
    RunWait,
    /// Run.
    Run,
}

impl AnimationFrameLoading {
    /// Project the selected control fields into their typed record.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        Ok(Self {
            x: keys.get(b'x', 0),
            y: keys.get(b'y', 0),
            create_frame: keys.get(b'c', 0),
            edit_frame: keys.get(b'r', 0),
            gap_ms: keys.get(b'z', 0),
            composition_mode: CompositionMode::parse(keys.get(b'X', 0))?,
            background: {
                let [r, g, b, a] = keys.get(b'Y', 0).to_le_bytes();
                Background { r, g, b, a }
            },
        })
    }
}

impl AnimationFrameComposition {
    /// Project the selected control fields into their typed record.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        Ok(Self {
            frame: keys.get(b'c', 0),
            edit_frame: keys.get(b'r', 0),
            x: keys.get(b'x', 0),
            y: keys.get(b'y', 0),
            width: keys.get(b'w', 0),
            height: keys.get(b'h', 0),
            left_edge: keys.get(b'X', 0),
            top_edge: keys.get(b'Y', 0),
            composition_mode: CompositionMode::parse(keys.get(b'C', 0))?,
        })
    }
}

impl AnimationControl {
    /// Project the selected control fields into their typed record.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        Ok(Self {
            frame: keys.get(b'r', 0),
            gap_ms: keys.get(b'z', 0),
            current_frame: keys.get(b'c', 0),
            loops: keys.get(b'v', 0),
            action: match keys.get(b's', 0) {
                0 => AnimationAction::Invalid,
                1 => AnimationAction::Stop,
                2 => AnimationAction::RunWait,
                3 => AnimationAction::Run,
                _ => return Err(ParseError::InvalidFormat),
            },
        })
    }
}

impl CompositionMode {
    /// Validate a compositing discriminant.
    fn parse(value: u32) -> Result<Self, ParseError> {
        match value {
            0 => Ok(Self::AlphaBlend),
            1 => Ok(Self::Overwrite),
            _ => Err(ParseError::InvalidFormat),
        }
    }
}

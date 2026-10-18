//! Terminal colour values.

/// An additive red, green and blue colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// The indexed terminal colour palette.
pub type Palette = [Rgb; 256];

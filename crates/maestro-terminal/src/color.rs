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

mod parse;
pub use parse::parse_palette_entry;

/// Invalid colour grammar or a palette index outside the byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The bytes do not follow the selected grammar.
    InvalidFormat,
    /// The numeric index cannot fit in a byte.
    Overflow,
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidFormat => "InvalidFormat",
            Self::Overflow => "Overflow",
        })
    }
}
impl std::error::Error for ParseError {}

/// A palette assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    /// Selected palette index.
    pub index: u8,
    /// Replacement colour.
    pub color: Rgb,
}

impl Rgb {
    /// Compare all three channels.
    #[must_use]
    pub fn eql(self, other: Self) -> bool {
        self == other
    }
    /// Write an eight-bit slash-separated colour.
    ///
    /// # Errors
    /// Propagates the writer's original I/O error.
    pub fn encode_rgb8<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "rgb:{:02x}/{:02x}/{:02x}", self.r, self.g, self.b)
    }
    /// Write a sixteen-bit colour with replicated eight-bit channels.
    ///
    /// # Errors
    /// Propagates the writer's original I/O error.
    pub fn encode_rgb16<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        write!(
            writer,
            "rgb:{:04x}/{:04x}/{:04x}",
            u16::from(self.r) * 257,
            u16::from(self.g) * 257,
            u16::from(self.b) * 257
        )
    }
}

mod names;
pub use names::{DEFAULT, Name, NoDefaultValue};
mod dynamic;
pub use dynamic::{DynamicPalette, DynamicRgb, PaletteMask};

mod targets;
pub use targets::{Dynamic, Special};

pub(crate) use parse::index;

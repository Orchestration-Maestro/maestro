//! Select-graphic-rendition attribute values.

/// The decoration used beneath text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Underline {
    /// No underline.
    #[default]
    None = 0,
    /// One solid line.
    Single = 1,
    /// Two solid lines.
    Double = 2,
    /// A wavy line.
    Curly = 3,
    /// A dotted line.
    Dotted = 4,
    /// A dashed line.
    Dashed = 5,
}

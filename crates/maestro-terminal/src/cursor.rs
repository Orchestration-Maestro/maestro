//! Visual shapes for the terminal cursor.

/// Cursor shape, independent of the blinking mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// Vertical bar.
    Bar,
    /// Filled character block.
    #[default]
    Block,
    /// Underline below the character.
    Underline,
    /// Outline of a character block.
    BlockHollow,
}

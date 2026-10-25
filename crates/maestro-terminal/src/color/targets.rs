//! Special and dynamic OSC colour targets.

/// Five special text colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    /// Bold text colour.
    Bold,
    /// Underline text colour.
    Underline,
    /// Blink text colour.
    Blink,
    /// Reverse text colour.
    Reverse,
    /// Italic text colour.
    Italic,
}
impl Special {
    /// Return the special target's index in the OSC4 namespace.
    #[must_use]
    pub const fn osc4(self) -> u16 {
        self as u16 + 256
    }
}

/// Dynamic colour targets, in their numeric protocol order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Dynamic {
    /// Foreground colour target.
    Foreground = 10,
    /// Background colour target.
    Background = 11,
    /// Cursor colour target.
    Cursor = 12,
    /// Pointer foreground colour target.
    PointerForeground = 13,
    /// Pointer background colour target.
    PointerBackground = 14,
    /// Tektronix foreground colour target.
    TektronixForeground = 15,
    /// Tektronix background colour target.
    TektronixBackground = 16,
    /// Highlight background colour target.
    HighlightBackground = 17,
    /// Tektronix cursor colour target.
    TektronixCursor = 18,
    /// Highlight foreground colour target.
    HighlightForeground = 19,
}
impl Dynamic {
    /// Advance to the next dynamic target, stopping after highlight foreground.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Foreground => Some(Self::Background),
            Self::Background => Some(Self::Cursor),
            Self::Cursor => Some(Self::PointerForeground),
            Self::PointerForeground => Some(Self::PointerBackground),
            Self::PointerBackground => Some(Self::TektronixForeground),
            Self::TektronixForeground => Some(Self::TektronixBackground),
            Self::TektronixBackground => Some(Self::HighlightBackground),
            Self::HighlightBackground => Some(Self::TektronixCursor),
            Self::TektronixCursor => Some(Self::HighlightForeground),
            Self::HighlightForeground => None,
        }
    }
}

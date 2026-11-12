//! Mutually exclusive terminal mouse reporting modes.

/// Mouse activities reported to the application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Event {
    /// Report no mouse activity.
    #[default]
    None,
    /// Report button presses.
    X10,
    /// Report button presses and releases.
    Normal,
    /// Also report motion while a button is pressed.
    Button,
    /// Report all pointer motion.
    Any,
}

/// Encoding used for reported mouse events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Legacy byte coordinates.
    #[default]
    X10,
    /// UTF-8 coordinates.
    Utf8,
    /// Decimal coordinates.
    Sgr,
    /// Extended decimal coordinates.
    Urxvt,
    /// Decimal pixel coordinates.
    SgrPixels,
}

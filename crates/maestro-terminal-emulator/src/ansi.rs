//! Terminal control-sequence state.

/// Most recently selected character-protection protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProtectedMode {
    /// No protection protocol has been selected.
    #[default]
    Off,
    /// ISO protection selected by escape sequences.
    Iso,
    /// DEC protection selected by control sequences.
    Dec,
}

/// Selected terminal text display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusDisplay {
    /// Normal screen content.
    #[default]
    Main,
    /// Separately addressed status line.
    StatusLine,
}

/// Requested visual cursor style, including the configured default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorStyle {
    /// Restore configured shape and blink.
    Default,
    /// Blinking block cursor.
    BlinkingBlock,
    /// Steady block cursor.
    SteadyBlock,
    /// Blinking underline cursor.
    BlinkingUnderline,
    /// Steady underline cursor.
    SteadyUnderline,
    /// Blinking vertical bar cursor.
    BlinkingBar,
    /// Steady vertical bar cursor.
    SteadyBar,
}

/// Requested keyboard escape-sequence format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifyKeyFormat {
    /// Legacy key encoding.
    Legacy,
    /// Modified cursor keys.
    CursorKeys,
    /// Modified function keys.
    FunctionKeys,
    /// Unmodified other keys.
    OtherKeysNone,
    /// Numeric other keys with conventional exceptions.
    OtherKeysNumericExcept,
    /// Numeric other keys without exceptions.
    OtherKeysNumeric,
}

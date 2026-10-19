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

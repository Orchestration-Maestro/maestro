//! Owned terminal input and supplied host geometry.

mod model;
pub(crate) mod parse;
/// Supplied host byte decoding for the multiplexer input loop.
pub mod raw_input;

pub use parse::parse_terminal_key_sequence;
pub use raw_input::parse_raw_input_bytes_sync;
/// Supplied geometry and mouse report operations.
pub mod mouse;

pub use model::{
    KeyIdentity, KeyboardProtocol, ModifyOtherKeysMode, MouseProtocolEncoding, MouseProtocolMode,
    TerminalKey, TextCommit, WindowsKeyRecord,
};

#[cfg(test)]
mod tests;

/// Decoded input routed by the daemon without performing host reads.
#[derive(Debug)]
pub enum RawInputEvent {
    /// Canonical key with its original source evidence.
    Key(TerminalKey),
    /// Committed text independent of a key.
    Text(TextCommit),
    /// Pasted text.
    Paste(String),
    /// Structured mouse input in cell coordinates.
    Mouse {
        /// Button or movement event.
        kind: maestro_protocol::frames::ClientMouseKind,
        /// Zero-based column.
        column: u16,
        /// Zero-based row.
        row: u16,
        /// Protocol modifier bits.
        modifiers: u8,
    },
    /// Host terminal gained focus.
    OuterFocusGained,
    /// Host terminal lost focus.
    OuterFocusLost,
    /// Observed host default colour.
    HostDefaultColor {
        /// Foreground or background selector.
        kind: crate::terminal_theme::DefaultColorKind,
        /// Observed colour.
        color: crate::terminal_theme::RgbColor,
    },
    /// Ordered observed palette entries.
    HostPaletteColors {
        /// Palette index and colour pairs.
        colors: Vec<(u8, crate::terminal_theme::RgbColor)>,
    },
    /// Observed host colour scheme.
    HostColorSchemeChanged(crate::terminal_theme::HostAppearance),
    /// Observed host cell dimensions.
    HostCellSizeReport {
        /// Cell width in pixels.
        width_px: u32,
        /// Cell height in pixels.
        height_px: u32,
    },
    /// Input without a supported decoded representation.
    Unsupported,
}

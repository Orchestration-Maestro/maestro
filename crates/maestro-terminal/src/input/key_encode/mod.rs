//! Keyboard bytes selected by the terminal's input modes.

mod kitty;
mod legacy;

use super::key::KeyEvent;
use super::key_mods::{Mod, Mods, Side};
use crate::modes::{Mode, ModeState};

/// How the macOS option modifier participates in terminal alt sequences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OptionAsAlt {
    /// Keep option's generated text.
    #[default]
    False,
    /// Treat both option keys as alt.
    True,
    /// Treat only the left option key as alt.
    Left,
    /// Treat only the right option key as alt.
    Right,
}

impl OptionAsAlt {
    /// Whether alt is a terminal modifier on this platform and side.
    pub(super) fn active(self, mods: Mods) -> bool {
        if !cfg!(target_os = "macos") {
            return true;
        }
        match self {
            Self::False => false,
            Self::True => true,
            Self::Left => mods.side(Mod::Alt) == Side::Left,
            Self::Right => mods.side(Mod::Alt) == Side::Right,
        }
    }
}

/// Terminal modes and caller configuration controlling keyboard bytes.
#[derive(Clone, Debug)]
pub struct KeyEncoder {
    /// An owned terminal-mode snapshot; later terminal changes do not affect it.
    pub modes: ModeState,
    /// Enhanced keyboard protocol flags; disabled for legacy encoding.
    pub kitty_flags: crate::kitty::key::Flags,
    /// Encode modified characters using extended modifier mode 2.
    pub modify_other_keys_state_2: bool,
    /// Caller-supplied macOS option policy; not determined by terminal modes.
    pub macos_option_as_alt: OptionAsAlt,
}

impl KeyEncoder {
    /// Snapshot the terminal's input modes and active-screen keyboard flags.
    /// The macOS option policy defaults to false and may be set by the caller.
    #[must_use]
    pub fn from_terminal(terminal: &crate::Terminal) -> Self {
        Self {
            modes: terminal.modes.clone(),
            kitty_flags: terminal.kitty_keyboard_flags(),
            modify_other_keys_state_2: terminal.modify_other_keys_enabled(),
            macos_option_as_alt: OptionAsAlt::False,
        }
    }

    /// Encode one event; releases and intermediate composition may produce no bytes.
    #[must_use]
    pub fn encode(&self, event: &KeyEvent) -> Vec<u8> {
        if self.kitty_flags == crate::kitty::key::Flags::DISABLED {
            legacy::encode(event, self)
        } else {
            kitty::encode(event, self)
        }
    }
}

impl Default for KeyEncoder {
    fn default() -> Self {
        let mut modes = ModeState::default();
        modes.set(Mode::IgnoreKeypadWithNumlock, false);
        modes.set(Mode::AltEscPrefix, false);
        Self {
            modes,
            kitty_flags: crate::kitty::key::Flags::DISABLED,
            modify_other_keys_state_2: false,
            macos_option_as_alt: OptionAsAlt::default(),
        }
    }
}

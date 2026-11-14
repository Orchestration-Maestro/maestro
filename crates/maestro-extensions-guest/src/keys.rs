//! Key vocabulary and decoding supplied by the installed native terminal owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult};
/// An open terminal key identifier.
pub type KeyId = String;
/// Enhanced keyboard event vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEventType {
    /// Initial key press.
    Press,
    /// Held-key repeat.
    Repeat,
    /// Key release.
    Release,
}
/// Named keys and native modifier constructors.
pub struct Key;
impl Key {
    /// The `escape` key.
    pub const ESCAPE: &'static str = "escape";
    /// The `esc` key.
    pub const ESC: &'static str = "esc";
    /// The `enter` key.
    pub const ENTER: &'static str = "enter";
    /// The `return` key.
    pub const RETURN: &'static str = "return";
    /// The `tab` key.
    pub const TAB: &'static str = "tab";
    /// The `space` key.
    pub const SPACE: &'static str = "space";
    /// The `backspace` key.
    pub const BACKSPACE: &'static str = "backspace";
    /// The `delete` key.
    pub const DELETE: &'static str = "delete";
    /// The `insert` key.
    pub const INSERT: &'static str = "insert";
    /// The `clear` key.
    pub const CLEAR: &'static str = "clear";
    /// The `home` key.
    pub const HOME: &'static str = "home";
    /// The `end` key.
    pub const END: &'static str = "end";
    /// The `pageUp` key.
    pub const PAGE_UP: &'static str = "pageUp";
    /// The `pageDown` key.
    pub const PAGE_DOWN: &'static str = "pageDown";
    /// The `up` key.
    pub const UP: &'static str = "up";
    /// The `down` key.
    pub const DOWN: &'static str = "down";
    /// The `left` key.
    pub const LEFT: &'static str = "left";
    /// The `right` key.
    pub const RIGHT: &'static str = "right";
    /// The `f1` key.
    pub const F1: &'static str = "f1";
    /// The `f2` key.
    pub const F2: &'static str = "f2";
    /// The `f3` key.
    pub const F3: &'static str = "f3";
    /// The `f4` key.
    pub const F4: &'static str = "f4";
    /// The `f5` key.
    pub const F5: &'static str = "f5";
    /// The `f6` key.
    pub const F6: &'static str = "f6";
    /// The `f7` key.
    pub const F7: &'static str = "f7";
    /// The `f8` key.
    pub const F8: &'static str = "f8";
    /// The `f9` key.
    pub const F9: &'static str = "f9";
    /// The `f10` key.
    pub const F10: &'static str = "f10";
    /// The `f11` key.
    pub const F11: &'static str = "f11";
    /// The `f12` key.
    pub const F12: &'static str = "f12";
    /// The `backtick` key.
    pub const BACKTICK: &'static str = "`";
    /// The `hyphen` key.
    pub const HYPHEN: &'static str = "-";
    /// The `equals` key.
    pub const EQUALS: &'static str = "=";
    /// The `leftbracket` key.
    pub const LEFTBRACKET: &'static str = "[";
    /// The `rightbracket` key.
    pub const RIGHTBRACKET: &'static str = "]";
    /// The `backslash` key.
    pub const BACKSLASH: &'static str = "\\";
    /// The `semicolon` key.
    pub const SEMICOLON: &'static str = ";";
    /// The `quote` key.
    pub const QUOTE: &'static str = "'";
    /// The `comma` key.
    pub const COMMA: &'static str = ",";
    /// The `period` key.
    pub const PERIOD: &'static str = ".";
    /// The `slash` key.
    pub const SLASH: &'static str = "/";
    /// The `exclamation` key.
    pub const EXCLAMATION: &'static str = "!";
    /// The `at` key.
    pub const AT: &'static str = "@";
    /// The `hash` key.
    pub const HASH: &'static str = "#";
    /// The `dollar` key.
    pub const DOLLAR: &'static str = "$";
    /// The `percent` key.
    pub const PERCENT: &'static str = "%";
    /// The `caret` key.
    pub const CARET: &'static str = "^";
    /// The `ampersand` key.
    pub const AMPERSAND: &'static str = "&";
    /// The `asterisk` key.
    pub const ASTERISK: &'static str = "*";
    /// The `leftparen` key.
    pub const LEFTPAREN: &'static str = "(";
    /// The `rightparen` key.
    pub const RIGHTPAREN: &'static str = ")";
    /// The `underscore` key.
    pub const UNDERSCORE: &'static str = "_";
    /// The `plus` key.
    pub const PLUS: &'static str = "+";
    /// The `pipe` key.
    pub const PIPE: &'static str = "|";
    /// The `tilde` key.
    pub const TILDE: &'static str = "~";
    /// The `leftbrace` key.
    pub const LEFTBRACE: &'static str = "{";
    /// The `rightbrace` key.
    pub const RIGHTBRACE: &'static str = "}";
    /// The `colon` key.
    pub const COLON: &'static str = ":";
    /// The `lessthan` key.
    pub const LESSTHAN: &'static str = "<";
    /// The `greaterthan` key.
    pub const GREATERTHAN: &'static str = ">";
    /// The `question` key.
    pub const QUESTION: &'static str = "?";
    /// Constructs `ctrl` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl", &(key,))
    }
    /// Constructs `shift` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn shift(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.shift", &(key,))
    }
    /// Constructs `alt` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn alt(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.alt", &(key,))
    }
    /// Constructs `super_key` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn super_key(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.super_key", &(key,))
    }
    /// Constructs `ctrl_shift` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl_shift(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl_shift", &(key,))
    }
    /// Constructs `shift_ctrl` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn shift_ctrl(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.shift_ctrl", &(key,))
    }
    /// Constructs `ctrl_alt` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl_alt(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl_alt", &(key,))
    }
    /// Constructs `alt_ctrl` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn alt_ctrl(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.alt_ctrl", &(key,))
    }
    /// Constructs `shift_alt` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn shift_alt(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.shift_alt", &(key,))
    }
    /// Constructs `alt_shift` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn alt_shift(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.alt_shift", &(key,))
    }
    /// Constructs `ctrl_super` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl_super(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl_super", &(key,))
    }
    /// Constructs `super_ctrl` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn super_ctrl(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.super_ctrl", &(key,))
    }
    /// Constructs `shift_super` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn shift_super(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.shift_super", &(key,))
    }
    /// Constructs `super_shift` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn super_shift(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.super_shift", &(key,))
    }
    /// Constructs `alt_super` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn alt_super(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.alt_super", &(key,))
    }
    /// Constructs `super_alt` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn super_alt(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.super_alt", &(key,))
    }
    /// Constructs `ctrl_shift_alt` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl_shift_alt(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl_shift_alt", &(key,))
    }
    /// Constructs `ctrl_shift_super` through the native key owner.
    ///
    /// # Errors
    /// Returns the host's failure or an invalid response.
    pub fn ctrl_shift_super(api: &ExtensionAPI, key: &str) -> ExtensionResult<KeyId> {
        api.helper("keys.ctrl_shift_super", &(key,))
    }
}

/// Changes the host's process-wide enhanced keyboard flag.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn set_kitty_protocol_active(api: &ExtensionAPI, active: bool) -> ExtensionResult<()> {
    api.helper("keys.set_kitty_protocol_active", &(active,))
}
/// Reads the shared host keyboard flag.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_kitty_protocol_active(api: &ExtensionAPI) -> ExtensionResult<bool> {
    api.helper("keys.is_kitty_protocol_active", &())
}

/// Delegates `parse_key` to the native terminal key owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn parse_key(api: &ExtensionAPI, data: &str) -> ExtensionResult<Option<KeyId>> {
    api.helper("keys.parse_key", &(data,))
}

/// Delegates `decode_kitty_printable` to the native terminal key owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn decode_kitty_printable(api: &ExtensionAPI, data: &str) -> ExtensionResult<Option<char>> {
    api.helper("keys.decode_kitty_printable", &(data,))
}

/// Delegates `decode_printable_key` to the native terminal key owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn decode_printable_key(api: &ExtensionAPI, data: &str) -> ExtensionResult<Option<char>> {
    api.helper("keys.decode_printable_key", &(data,))
}

/// Delegates `is_key_release` to the native terminal key owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_key_release(api: &ExtensionAPI, data: &str) -> ExtensionResult<bool> {
    api.helper("keys.is_key_release", &(data,))
}

/// Delegates `is_key_repeat` to the native terminal key owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_key_repeat(api: &ExtensionAPI, data: &str) -> ExtensionResult<bool> {
    api.helper("keys.is_key_repeat", &(data,))
}

/// Matches a literal key identifier with its exact modifiers through the host.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn matches_key(api: &ExtensionAPI, data: &str, key_id: &str) -> ExtensionResult<bool> {
    api.helper("keys.matches_key", &(data, key_id))
}

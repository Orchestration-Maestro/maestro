#![doc = include_str!("../../../../docs/terminal/keys.md")]

mod kitty;
mod legacy;
mod names;
mod printable;

use std::sync::atomic::{AtomicBool, Ordering};

use kitty::{ModifyOtherKeys, Sequence};
use names::{ALT, CTRL, Code, Identifier, KEYPAD_ENTER, Named, SHIFT, Target, raw_ctrl_char};
pub use names::{Key, KeyId};

/// The phase of an enhanced key report: the key going down, repeating or going up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEventType {
    /// The key went down.
    Press,
    /// The key is held down and repeating.
    Repeat,
    /// The key went up.
    Release,
}

/// Whether the terminal runs the enhanced keyboard protocol.
static KITTY_PROTOCOL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Records whether the terminal runs the enhanced keyboard protocol.
///
/// The flag starts `false`. [`matches_key`] and [`parse_key`] read it when they
/// interpret legacy input, because the protocol changes what a few legacy inputs mean;
/// the typed-text and event functions never read it.
pub fn set_kitty_protocol_active(active: bool) {
    KITTY_PROTOCOL_ACTIVE.store(active, Ordering::Relaxed);
}

/// Whether the terminal was recorded as running the enhanced keyboard protocol.
#[must_use]
pub fn is_kitty_protocol_active() -> bool {
    KITTY_PROTOCOL_ACTIVE.load(Ordering::Relaxed)
}

/// One input, decoded once, and the protocol state the matching rules read it under.
struct Input<'a> {
    /// The raw terminal input.
    data: &'a str,
    /// Whether the enhanced protocol is active.
    kitty: bool,
    /// The input as an enhanced report whose fields are valid.
    enhanced: Option<Sequence>,
    /// The input as a modifyOtherKeys report.
    other: Option<ModifyOtherKeys>,
}

impl<'a> Input<'a> {
    /// Decodes `data` under the current protocol state.
    fn new(data: &'a str) -> Self {
        Self {
            data,
            kitty: is_kitty_protocol_active(),
            enhanced: Sequence::parse(data),
            other: ModifyOtherKeys::parse(data),
        }
    }

    /// Whether the input is an enhanced report of `code` held with `modifier`, ignoring
    /// lock bits.
    fn enhanced(&self, code: Code, modifier: u32) -> bool {
        self.enhanced
            .as_ref()
            .is_some_and(|sequence| sequence.matches(code, modifier))
    }

    /// Whether the input is a modifyOtherKeys report of `key` held with exactly `modifier`,
    /// lock bits included.
    fn other(&self, key: char, modifier: u32) -> bool {
        self.other
            .is_some_and(|report| report.matches(key, modifier))
    }

    /// Whether the input is a modifyOtherKeys report of a printable `key` with modifiers.
    fn other_printable(&self, key: char, modifier: u32) -> bool {
        self.other
            .is_some_and(|report| report.matches_printable(key, modifier))
    }

    /// Whether the input is a named key held with exactly `modifier`.
    fn matches_named(&self, key: Named, modifier: u32) -> bool {
        if key == Named::Escape && modifier != 0 {
            return false;
        }
        let code = key.code();
        let enhanced = code.is_some_and(|code| self.enhanced(code, modifier));
        let legacy = match key {
            Named::Escape => modifier == 0 && self.data == "\x1b",
            Named::Space => self.matches_space(modifier),
            Named::Tab => {
                (modifier == SHIFT && self.data == "\x1b[Z") || (modifier == 0 && self.data == "\t")
            }
            Named::Enter => self.matches_enter(modifier),
            Named::Backspace => self.matches_backspace(modifier),
            _ => legacy::matches(self.data, key, modifier, self.kitty),
        };
        legacy || enhanced || self.matches_other(key, modifier)
    }

    /// Whether the input is a modifyOtherKeys report of a named key held with `modifier`.
    ///
    /// Plain tab and enter reports are not recognized in this form.
    fn matches_other(&self, key: Named, modifier: u32) -> bool {
        match key {
            Named::Escape | Named::Space | Named::Backspace => {
                matches!(key.code(), Some(Code::Char(c)) if self.other(c, modifier))
            }
            Named::Tab | Named::Enter => {
                modifier != 0
                    && matches!(key.code(), Some(Code::Char(c)) if self.other(c, modifier))
            }
            _ => false,
        }
    }

    /// Whether the input is the space bar held with exactly `modifier`.
    fn matches_space(&self, modifier: u32) -> bool {
        let legacy_control = !self.kitty
            && ((modifier == CTRL && self.data == "\0")
                || (modifier == ALT && self.data == "\x1b "));
        legacy_control || (modifier == 0 && self.data == " ")
    }

    /// Whether the input is enter held with exactly `modifier`.
    fn matches_enter(&self, modifier: u32) -> bool {
        let data = self.data;
        let keypad = self.enhanced(Code::Char(KEYPAD_ENTER), modifier);
        keypad
            || match modifier {
                SHIFT => self.kitty && matches!(data, "\x1b\r" | "\n"),
                ALT => !self.kitty && data == "\x1b\r",
                0 => data == "\r" || (!self.kitty && data == "\n") || data == "\x1bOM",
                _ => false,
            }
    }

    /// Whether the input is backspace held with exactly `modifier`.
    fn matches_backspace(&self, modifier: u32) -> bool {
        match modifier {
            ALT => matches!(self.data, "\x1b\x7f" | "\x1b\x08"),
            0 | CTRL => legacy::matches_raw_backspace(self.data, modifier),
            _ => false,
        }
    }

    /// Whether the input is a lowercase letter, digit or symbol held with exactly `modifier`.
    fn matches_char(&self, key: char, modifier: u32) -> bool {
        self.matches_char_control(key, modifier)
            || self.enhanced(Code::Char(key), modifier)
            || (modifier != 0 && self.other_printable(key, modifier))
    }

    /// Whether the input is the legacy control-byte form of `key` held with `modifier`.
    fn matches_char_control(&self, key: char, modifier: u32) -> bool {
        let sole = |text: &str| {
            let mut chars = text.chars();
            chars.next().filter(|_| chars.next().is_none())
        };
        let escaped = self.data.strip_prefix('\x1b').and_then(sole);
        let raw_ctrl = raw_ctrl_char(key);
        match modifier {
            0 => sole(self.data) == Some(key),
            SHIFT => key.is_ascii_lowercase() && sole(self.data) == Some(key.to_ascii_uppercase()),
            CTRL => raw_ctrl.is_some() && sole(self.data) == raw_ctrl,
            m if m == CTRL | ALT => !self.kitty && raw_ctrl.is_some() && escaped == raw_ctrl,
            ALT => {
                !self.kitty
                    && (key.is_ascii_lowercase() || key.is_ascii_digit())
                    && escaped == Some(key)
            }
            _ => false,
        }
    }
}

/// Whether `data` is the key named by `key_id`, held with exactly the modifiers it names.
///
/// The encodings recognized are those of the module page: legacy bytes and sequences,
/// enhanced reports and modifyOtherKeys reports, some only in one protocol state. Caps
/// Lock and Num Lock bits in an enhanced report are ignored; a modifyOtherKeys report
/// must carry exactly the identifier's modifier bits, so one with a lock bit set
/// matches nothing. An identifier that names no key, or that the input cannot express,
/// never matches. A release report matches its key; check [`is_key_release`] to skip
/// releases.
#[must_use]
pub fn matches_key(data: &str, key_id: &str) -> bool {
    let Some(identifier) = Identifier::parse(key_id) else {
        return false;
    };
    let input = Input::new(data);
    match identifier.target {
        Target::Named(key) => input.matches_named(key, identifier.modifier),
        Target::Char(key) => input.matches_char(key, identifier.modifier),
    }
}

/// The identifier of the key `data` reports, with modifiers in the order shift, ctrl, alt,
/// super, or `None` when it reports no key this module names.
#[must_use]
pub fn parse_key(data: &str) -> Option<KeyId> {
    if let Some(sequence) = Sequence::parse(data) {
        return sequence.name();
    }
    if let Some(report) = ModifyOtherKeys::parse(data) {
        return report.name();
    }
    legacy::parse(data, is_kitty_protocol_active())
}

/// The character typed by an enhanced `CSI u` report of a plain or shifted text key.
///
/// Caps Lock and Num Lock do not prevent a character. Returns `None` for any other input,
/// for reports with alt, ctrl, super or another modifier bit held, and for control
/// characters and functional keys.
#[must_use]
pub fn decode_kitty_printable(data: &str) -> Option<char> {
    printable::decode_kitty(data)
}

/// The character typed by an enhanced `CSI u` report or an xterm modifyOtherKeys report
/// of a plain or shifted text key.
///
/// The modifyOtherKeys form keeps private-use characters as text and, like the other
/// form, ignores the lock bits; both forms return `None` for control characters.
#[must_use]
pub fn decode_printable_key(data: &str) -> Option<char> {
    printable::decode_kitty(data).or_else(|| printable::decode_modify_other_keys(data))
}

/// Whether `data` is exactly one enhanced key report whose event is a release.
///
/// Text that merely contains such a pattern, and anything holding a bracketed-paste
/// start, is not a release.
#[must_use]
pub fn is_key_release(data: &str) -> bool {
    event_type(data) == Some(KeyEventType::Release)
}

/// Whether `data` is exactly one enhanced key report whose event is a repeat.
///
/// Text that merely contains such a pattern, and anything holding a bracketed-paste
/// start, is not a repeat.
#[must_use]
pub fn is_key_repeat(data: &str) -> bool {
    event_type(data) == Some(KeyEventType::Repeat)
}

/// The phase `data` reports, absent unless it is one complete report outside pasted text.
fn event_type(data: &str) -> Option<KeyEventType> {
    if data.contains("\x1b[200~") {
        return None;
    }
    kitty::event_type(data)
}

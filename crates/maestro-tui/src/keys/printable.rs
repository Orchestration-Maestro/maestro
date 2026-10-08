//! Text typed by an enhanced key report.

use super::kitty::{ModifyOtherKeys, Sequence};
use super::names::{Code, LOCKS, SHIFT};

/// The code points the enhanced protocol reserves for functional keys.
const FUNCTIONAL_KEYS: std::ops::RangeInclusive<char> = '\u{e000}'..='\u{f8ff}';

/// The character typed by an enhanced CSI-u report, if it is plain or shifted text.
///
/// Caps Lock and Num Lock are ignored. Reports with alt, ctrl, super or another modifier
/// bit held, control characters, navigation keypad codes and the reserved functional range
/// type nothing.
pub(super) fn decode_kitty(data: &str) -> Option<char> {
    let sequence = Sequence::parse(data)?;
    let Code::Char(code) = sequence.code else {
        return None;
    };
    if sequence.modifier & !(SHIFT | LOCKS) != 0 {
        return None;
    }
    let typed = match sequence.shifted {
        Some(shifted) if sequence.modifier & SHIFT != 0 => shifted,
        _ => code,
    };
    match Code::Char(typed).normalized() {
        Code::Char(c) if !c.is_control() && !FUNCTIONAL_KEYS.contains(&c) => Some(c),
        _ => None,
    }
}

/// The character typed by an xterm modifyOtherKeys report, if it is plain or shifted text.
///
/// Caps Lock and Num Lock are ignored; control characters type nothing.
pub(super) fn decode_modify_other_keys(data: &str) -> Option<char> {
    let report = ModifyOtherKeys::parse(data)?;
    (report.modifier & !(SHIFT | LOCKS) == 0 && !report.key.is_control()).then_some(report.key)
}

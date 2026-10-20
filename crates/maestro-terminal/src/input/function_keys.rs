//! PC-style function-key and application-keypad sequences.

use super::key::Key;
use super::key_encode::KeyEncoder;
use super::key_mods::Mods;
use crate::modes::Mode;

/// Encode shift, alt, control, super, Hyper and Meta in protocol order, plus one.
#[must_use]
pub(super) fn modifier_code(mods: Mods) -> u16 {
    let bits = mods.binding().int();
    1 + (bits & 1) + ((bits & 4) >> 1) + ((bits & 2) << 1) + (bits & 8) + ((mods.int() >> 6) & 0x30)
}

/// Select a PC-style sequence before considering generated text.
pub(super) fn sequence(key: Key, mods: Mods, opts: &KeyEncoder) -> Option<String> {
    let code = modifier_code(mods);
    if let Some((number, suffix)) = function_key(key) {
        return Some(function_sequence(number, suffix, code, opts));
    }
    if let Some(suffix) = keypad_key(key) {
        if opts.modes.get(Mode::KeypadKeys) && !opts.modes.get(Mode::IgnoreKeypadWithNumlock) {
            return Some(if code == 1 {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1bO{code}{suffix}")
            });
        }
        return (key == Key::NumpadEnter).then(|| "\r".into());
    }
    special_key(key, code, opts)
}

/// Cursor and function-key parameters; zero denotes a cursor key.
fn function_key(key: Key) -> Option<(u8, char)> {
    Some(match key {
        Key::ArrowUp | Key::NumpadUp => (0, 'A'),
        Key::ArrowDown | Key::NumpadDown => (0, 'B'),
        Key::ArrowRight | Key::NumpadRight => (0, 'C'),
        Key::ArrowLeft | Key::NumpadLeft => (0, 'D'),
        Key::NumpadBegin => (0, 'E'),
        Key::Home | Key::NumpadHome => (0, 'H'),
        Key::End | Key::NumpadEnd => (0, 'F'),
        Key::Insert | Key::NumpadInsert => (2, '~'),
        Key::Delete | Key::NumpadDelete => (3, '~'),
        Key::PageUp | Key::NumpadPageUp => (5, '~'),
        Key::PageDown | Key::NumpadPageDown => (6, '~'),
        Key::F1 => (1, 'P'),
        Key::F2 => (1, 'Q'),
        Key::F3 => (13, '~'),
        Key::F4 => (1, 'S'),
        Key::F5 => (15, '~'),
        Key::F6 => (17, '~'),
        Key::F7 => (18, '~'),
        Key::F8 => (19, '~'),
        Key::F9 => (20, '~'),
        Key::F10 => (21, '~'),
        Key::F11 => (23, '~'),
        Key::F12 => (24, '~'),
        _ => return None,
    })
}

/// Format a cursor or numbered function-key entry.
fn function_sequence(number: u8, suffix: char, code: u16, opts: &KeyEncoder) -> String {
    if code != 1 {
        return format!("\x1b[{};{code}{suffix}", number.max(1));
    }
    if number == 0 {
        let prefix = if opts.modes.get(Mode::CursorKeys) {
            'O'
        } else {
            '['
        };
        return format!("\x1b{prefix}{suffix}");
    }
    if number == 1 {
        return format!("\x1bO{suffix}");
    }
    if number == 13 {
        return "\x1bOR".into();
    }
    format!("\x1b[{number}{suffix}")
}

/// Application-keypad suffixes, before the normal numeric text fallback.
fn keypad_key(key: Key) -> Option<char> {
    Some(match key {
        Key::Numpad0 => 'p',
        Key::Numpad1 => 'q',
        Key::Numpad2 => 'r',
        Key::Numpad3 => 's',
        Key::Numpad4 => 't',
        Key::Numpad5 => 'u',
        Key::Numpad6 => 'v',
        Key::Numpad7 => 'w',
        Key::Numpad8 => 'x',
        Key::Numpad9 => 'y',
        Key::NumpadDecimal => 'n',
        Key::NumpadDivide => 'o',
        Key::NumpadMultiply => 'j',
        Key::NumpadSubtract => 'm',
        Key::NumpadAdd => 'k',
        Key::NumpadEnter => 'M',
        _ => return None,
    })
}

/// Escape, return, tab and backspace entries, including modifier exceptions.
fn special_key(key: Key, code: u16, opts: &KeyEncoder) -> Option<String> {
    let byte = match key {
        Key::Backspace => return Some(backspace(code, opts)),
        Key::Tab => b'\t',
        Key::Enter => b'\r',
        Key::Escape => 0x1b,
        _ => return None,
    };
    if key == Key::Enter && opts.encoding == super::key_encode::KeyEncoding::Semantic {
        return Some(if (code - 1) & 2 != 0 { "\x1b\r" } else { "\r" }.into());
    }
    if code == 1 {
        return Some(char::from(byte).into());
    }
    if key == Key::Tab && code == 5 && opts.encoding == super::key_encode::KeyEncoding::Semantic {
        return Some("\t".into());
    }
    if key == Key::Escape && code == 3 {
        return Some("\x1b\x1b".into());
    }
    if !opts.modify_other_keys_state_2 {
        if key == Key::Tab && code == 2 {
            return Some("\x1b[Z".into());
        }
        if matches!(key, Key::Tab | Key::Enter) && code == 3 {
            return Some(format!("\x1b{}", char::from(byte)));
        }
    }
    Some(format!("\x1b[27;{code};{byte}~"))
}

/// Backarrow mode affects only the default and control-only entries.
fn backspace(code: u16, opts: &KeyEncoder) -> String {
    if code == 1 || code == 5 {
        let bs = (code == 5) != opts.modes.get(Mode::BackarrowKeyMode);
        return char::from(if bs { 8 } else { 127 }).into();
    }
    if opts.modify_other_keys_state_2 {
        return format!("\x1b[27;{code};127~");
    }
    if code == 8 {
        return char::from(if opts.modes.get(Mode::BackarrowKeyMode) {
            8
        } else {
            127
        })
        .into();
    }
    let bits = code - 1;
    let byte = char::from(if bits & 4 != 0 { 8 } else { 127 });
    if bits & 2 != 0 {
        format!("\x1b{byte}")
    } else {
        byte.into()
    }
}

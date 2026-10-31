//! Workspace key-combination parsing and matching.

use maestro_protocol::frames::{ClientKeyCode, KeyModifiers};
use maestro_tui::keys::{Identifier, Named, Target};

/// Semantic key code with protocol modifier bits.
pub type KeyCombo = (ClientKeyCode, u8);

/// Parses one key-combination spelling through the toolkit's key grammar, rejecting multiple
/// or absent keys.
#[must_use]
pub fn parse_key_combo(text: &str) -> Option<KeyCombo> {
    let identifier = Identifier::parse(text)?;
    let mut modifiers = project_modifiers(identifier.modifier);
    let code = match identifier.target {
        Target::Char(character) => {
            if names_capital(text) {
                modifiers |= KeyModifiers::SHIFT;
            }
            ClientKeyCode::Char(character)
        }
        Target::Named(named) => project_named(named)?,
    };
    Some(normalize_key_combo((code, modifiers)))
}

/// Whether the key of `text` is one ASCII capital, which the workspace reads as shifted.
fn names_capital(text: &str) -> bool {
    let key = text.rsplit('+').next().map_or("", str::trim);
    let mut characters = key.chars();
    matches!((characters.next(), characters.next()), (Some(c), None) if c.is_ascii_uppercase())
}

/// Maps the toolkit's modifier bits onto the protocol's.
fn project_modifiers(modifier: u32) -> u8 {
    [
        (Identifier::SHIFT, KeyModifiers::SHIFT),
        (Identifier::CTRL, KeyModifiers::CONTROL),
        (Identifier::ALT, KeyModifiers::ALT),
        (Identifier::SUPER, KeyModifiers::SUPER),
        (Identifier::HYPER, KeyModifiers::HYPER),
    ]
    .into_iter()
    .filter(|(bit, _)| modifier & bit != 0)
    .fold(KeyModifiers::NONE, |bits, (_, protocol)| bits | protocol)
}

/// Maps a named toolkit key onto the protocol's key code, absent for keys it has no code for.
fn project_named(named: Named) -> Option<ClientKeyCode> {
    Some(match named {
        Named::Escape => ClientKeyCode::Esc,
        Named::Tab => ClientKeyCode::Tab,
        Named::BackTab => ClientKeyCode::BackTab,
        Named::Enter => ClientKeyCode::Enter,
        Named::Space => ClientKeyCode::Char(' '),
        Named::Backspace => ClientKeyCode::Backspace,
        Named::Insert => ClientKeyCode::Insert,
        Named::Delete => ClientKeyCode::Delete,
        Named::Home => ClientKeyCode::Home,
        Named::End => ClientKeyCode::End,
        Named::PageUp => ClientKeyCode::PageUp,
        Named::PageDown => ClientKeyCode::PageDown,
        Named::Up => ClientKeyCode::Up,
        Named::Down => ClientKeyCode::Down,
        Named::Left => ClientKeyCode::Left,
        Named::Right => ClientKeyCode::Right,
        Named::Function(number) => ClientKeyCode::F(number),
        _ => return None,
    })
}

/// Canonicalizes shifted Tab as `BackTab` without a redundant Shift modifier.
#[must_use]
pub fn normalize_key_combo((code, modifiers): KeyCombo) -> KeyCombo {
    let (normalized, modifiers) = normalized_key_parts(&code, modifiers);
    let backtab = *normalized == ClientKeyCode::BackTab;
    (
        if backtab {
            ClientKeyCode::BackTab
        } else {
            code
        },
        modifiers,
    )
}

/// Borrows a canonical code without copying the terminal's input record.
fn normalized_key_parts(code: &ClientKeyCode, modifiers: u8) -> (&ClientKeyCode, u8) {
    if *code == ClientKeyCode::BackTab
        || (*code == ClientKeyCode::Tab && modifiers & KeyModifiers::SHIFT != 0)
    {
        (&ClientKeyCode::BackTab, modifiers & !KeyModifiers::SHIFT)
    } else {
        (code, modifiers)
    }
}

/// Formats a combination using the workspace's canonical modifier order.
#[must_use]
pub fn format_key_combo((code, modifiers): KeyCombo) -> String {
    let super_label = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "super"
    };
    let mut parts: Vec<String> = [
        (KeyModifiers::CONTROL, "ctrl"),
        (KeyModifiers::ALT, "alt"),
        (KeyModifiers::SHIFT, "shift"),
        (KeyModifiers::SUPER, super_label),
        (KeyModifiers::HYPER, "hyper"),
        (KeyModifiers::META, "meta"),
    ]
    .into_iter()
    .filter(|(modifier, _)| {
        modifiers & modifier != 0
            && !(*modifier == KeyModifiers::SHIFT && code == ClientKeyCode::BackTab)
    })
    .map(|(_, name)| name.to_owned())
    .collect();
    let key = match code {
        ClientKeyCode::Char(' ') => "space".into(),
        ClientKeyCode::Char(character) => character.to_string(),
        ClientKeyCode::Enter => "enter".into(),
        ClientKeyCode::Esc => "esc".into(),
        ClientKeyCode::Tab => "tab".into(),
        ClientKeyCode::BackTab => "shift+tab".into(),
        ClientKeyCode::Backspace => "backspace".into(),
        ClientKeyCode::Left => "left".into(),
        ClientKeyCode::Right => "right".into(),
        ClientKeyCode::Up => "up".into(),
        ClientKeyCode::Down => "down".into(),
        ClientKeyCode::F(number) => format!("f{number}"),
        code => format!("{code:?}").to_lowercase(),
    };
    parts.push(key);
    parts.join("+")
}

/// Matches a decoded terminal key, including alternate shifted code points.
#[must_use]
pub fn terminal_key_matches_combo(key: &crate::input::TerminalKey, combo: KeyCombo) -> bool {
    let (actual, modifiers) = normalized_key_parts(&key.code, key.modifiers);
    let (expected, expected_modifiers) = normalize_key_combo(combo);
    if modifiers == expected_modifiers
        && codes_match(actual, &expected, modifiers, key.shifted_codepoint)
    {
        return true;
    }
    if modifiers & KeyModifiers::SHIFT != 0
        && modifiers & !KeyModifiers::SHIFT == expected_modifiers
        && shifted_char_matches_expected(actual, key.shifted_codepoint, &expected)
    {
        return true;
    }
    matches!((actual, expected), (ClientKeyCode::Char(actual), ClientKeyCode::Char(expected))
        if modifiers & KeyModifiers::SHIFT == 0
            && actual.is_ascii_uppercase()
            && expected.is_ascii_lowercase()
            && actual.to_ascii_lowercase() == expected
            && modifiers | KeyModifiers::SHIFT == expected_modifiers)
}

/// Compares canonical codes under equal modifier sets.
fn codes_match(
    actual: &ClientKeyCode,
    expected: &ClientKeyCode,
    modifiers: u8,
    shifted: Option<u32>,
) -> bool {
    match (actual, expected) {
        (ClientKeyCode::Char(actual), ClientKeyCode::Char(expected))
            if actual.is_ascii_alphabetic() && expected.is_ascii_alphabetic() =>
        {
            actual == expected
                || modifiers & KeyModifiers::SHIFT != 0 && actual.eq_ignore_ascii_case(expected)
        }
        (ClientKeyCode::Char(_), ClientKeyCode::Char(_)) => {
            actual == expected || shifted_char_matches_expected(actual, shifted, expected)
        }
        _ => actual == expected,
    }
}

/// Matches the alternate code point or an already shifted punctuation key.
fn shifted_char_matches_expected(
    actual: &ClientKeyCode,
    shifted: Option<u32>,
    expected: &ClientKeyCode,
) -> bool {
    let ClientKeyCode::Char(expected) = expected else {
        return false;
    };
    if let Some(shifted) = shifted.and_then(char::from_u32) {
        return shifted == *expected;
    }
    matches!(actual, ClientKeyCode::Char(actual)
        if actual == expected && "!@#$%^&*()_+{}|:\"<>?~".contains(*expected))
}

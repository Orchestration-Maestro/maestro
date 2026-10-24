//! Workspace key-combination parsing and matching.

use maestro_protocol::frames::{ClientKeyCode, KeyModifiers};

/// Semantic key code with protocol modifier bits.
pub type KeyCombo = (ClientKeyCode, u8);

/// Parses one key-combination spelling, rejecting multiple or absent keys.
#[must_use]
pub fn parse_key_combo(text: &str) -> Option<KeyCombo> {
    let mut modifiers = KeyModifiers::NONE;
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        if part.is_empty() {
            return None;
        }
        if let Some(modifier) = parse_modifier_token(part) {
            modifiers |= modifier;
        } else if key.replace(part).is_some() {
            return None;
        }
    }
    let mut code = parse_key_code(key?)?;
    if let ClientKeyCode::Char(character) = &mut code
        && character.is_ascii_uppercase()
    {
        *character = character.to_ascii_lowercase();
        modifiers |= KeyModifiers::SHIFT;
    }
    Some(normalize_key_combo((code, modifiers)))
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

/// Resolves workspace key names or a single Unicode scalar.
fn parse_key_code(key: &str) -> Option<ClientKeyCode> {
    let code = match key.to_lowercase().as_str() {
        "space" | " " => ClientKeyCode::Char(' '),
        "enter" | "return" => ClientKeyCode::Enter,
        "esc" | "escape" => ClientKeyCode::Esc,
        "tab" => ClientKeyCode::Tab,
        "backspace" | "bs" => ClientKeyCode::Backspace,
        "left" => ClientKeyCode::Left,
        "right" => ClientKeyCode::Right,
        "up" => ClientKeyCode::Up,
        "down" => ClientKeyCode::Down,
        "minus" => ClientKeyCode::Char('-'),
        "comma" => ClientKeyCode::Char(','),
        "period" => ClientKeyCode::Char('.'),
        "slash" => ClientKeyCode::Char('/'),
        "backslash" => ClientKeyCode::Char('\\'),
        "quote" => ClientKeyCode::Char('\''),
        "double_quote" | "double-quote" => ClientKeyCode::Char('"'),
        "semicolon" => ClientKeyCode::Char(';'),
        "colon" => ClientKeyCode::Char(':'),
        "percent" => ClientKeyCode::Char('%'),
        "ampersand" => ClientKeyCode::Char('&'),
        "backtick" => ClientKeyCode::Char('`'),
        "plus" => ClientKeyCode::Char('+'),
        name if name.len() > 1 && name.starts_with('f') => {
            ClientKeyCode::F(name[1..].parse().ok()?)
        }
        _ => {
            let mut chars = key.chars();
            let character = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            ClientKeyCode::Char(character)
        }
    };
    Some(code)
}

/// Recognizes the workspace modifier aliases independently of key spelling.
fn parse_modifier_token(token: &str) -> Option<u8> {
    match token.to_lowercase().as_str() {
        "ctrl" | "control" => Some(KeyModifiers::CONTROL),
        "shift" => Some(KeyModifiers::SHIFT),
        "alt" | "option" | "meta" => Some(KeyModifiers::ALT),
        "cmd" | "command" | "super" => Some(KeyModifiers::SUPER),
        "hyper" => Some(KeyModifiers::HYPER),
        _ => None,
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

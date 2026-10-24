//! Legacy text, control-byte and extended-modifier encoding.

use super::KeyEncoder;
use crate::input::function_keys::{modifier_code, sequence};
use crate::input::key_mods::Mods;
use crate::input::keyboard::{Key, KeyAction, KeyEvent};
use crate::modes::Mode;

/// Encode traditional terminal input, retaining function-key precedence.
pub(super) fn encode(event: &KeyEvent, opts: &KeyEncoder) -> Vec<u8> {
    if event.action == KeyAction::Release || event.composing {
        return Vec::new();
    }
    if let Some(bytes) = function_sequence(event, opts) {
        return bytes;
    }
    if let Some(byte) = ctrl_sequence(event, opts) {
        return if event.effective_mods().contains(Mods::ALT) {
            vec![0x1b, byte]
        } else {
            vec![byte]
        };
    }
    if !event.utf8.is_empty() {
        if let Some(bytes) = modified(event, opts) {
            return bytes;
        }
        if let Some(bytes) = fixterms(event) {
            return bytes;
        }
    }
    if let Some(bytes) = alt_prefix(event, opts) {
        return bytes;
    }
    if cfg!(target_os = "macos") && event.mods.contains(Mods::SUPER) {
        return Vec::new();
    }
    event.utf8.as_bytes().to_vec()
}

/// A single control character is not committed composition text.
pub(super) fn control_text(text: &str) -> bool {
    text.len() == 1 && text.as_bytes()[0].is_ascii_control()
}

/// Committed IME text bypasses enter/escape; preedit backspace emits nothing.
fn function_sequence(event: &KeyEvent, opts: &KeyEncoder) -> Option<Vec<u8>> {
    let bytes = sequence(event.key, event.mods, opts)?;
    if !event.utf8.is_empty() && !control_text(&event.utf8) {
        match event.key {
            Key::Backspace => return Some(Vec::new()),
            Key::Enter | Key::Escape => return None,
            _ => {}
        }
    }
    Some(bytes.into_bytes())
}

/// Find the one codepoint required by modifier protocols.
fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

/// Extended modifier mode uses all binding modifiers, even consumed shift.
fn modified(event: &KeyEvent, opts: &KeyEncoder) -> Option<Vec<u8>> {
    if !opts.modify_other_keys_state_2 {
        return None;
    }
    let cp = single_char(&event.utf8)?;
    let mut mods = event.mods.binding();
    if !opts.macos_option_as_alt.active(event.mods) {
        mods = mods.unset(Mods::ALT);
    }
    let should_modify =
        ('@'..='\x7f').contains(&cp) || !mods.unset(Mods::SHIFT).empty() || cp == ' ';
    if mods.empty() || !should_modify {
        return None;
    }
    Some(format!("\x1b[27;{};{}~", modifier_code(mods), u32::from(cp)).into_bytes())
}

/// Distinguish control combinations without an unambiguous C0 encoding.
fn fixterms(event: &KeyEvent) -> Option<Vec<u8>> {
    if !event.mods.contains(Mods::CTRL) {
        return None;
    }
    let mut cp = single_char(&event.utf8)?;
    let mut mods = event.mods.unset(Mods::SUPER).binding();
    if mods.contains(Mods::SHIFT) && cp.is_ascii_uppercase() {
        cp = cp.to_ascii_lowercase();
    }
    if event.unshifted_codepoint != u32::from(cp) {
        mods = mods.unset(Mods::SHIFT);
    }
    Some(format!("\x1b[{};{}u", u32::from(cp), modifier_code(mods)).into_bytes())
}

/// ESC-prefix semantic text, or a physical single-byte key identity.
fn alt_prefix(event: &KeyEvent, opts: &KeyEncoder) -> Option<Vec<u8>> {
    if !event.effective_mods().contains(Mods::ALT)
        || !opts.modes.get(Mode::AltEscPrefix)
        || !opts.macos_option_as_alt.active(event.mods)
    {
        return None;
    }
    let mut bytes = vec![0x1b];
    if opts.encoding == super::KeyEncoding::Semantic {
        bytes.extend_from_slice(event.utf8.as_bytes());
    } else {
        let byte = if event.utf8.len() == 1 {
            event.utf8.as_bytes()[0]
        } else {
            u8::try_from(event.unshifted_codepoint)
                .ok()
                .filter(|&cp| cp > 0)?
        };
        bytes.push(byte);
    }
    Some(bytes)
}

/// Convert only control bindings whose character has a traditional C0 value.
fn ctrl_sequence(event: &KeyEvent, opts: &KeyEncoder) -> Option<u8> {
    if !event.mods.contains(Mods::CTRL) {
        return None;
    }
    if opts.encoding == super::KeyEncoding::Semantic {
        let byte = u8::try_from(event.unshifted_codepoint).ok()?;
        return Some(match byte.to_ascii_lowercase() {
            b'a'..=b'z' => byte.to_ascii_lowercase() - b'a' + 1,
            b'[' => 27,
            b'-' => 31,
            b'?' | b'8' => byte,
            _ => control_byte(byte).unwrap_or(byte),
        });
    }
    let mut mods = event.mods.binding().unset(Mods::ALT);
    let mut byte = if event.utf8.len() == 1 {
        event.utf8.as_bytes()[0]
    } else {
        if mods != Mods::CTRL {
            return None;
        }
        u8::try_from(u32::from(event.key.codepoint()?)).ok()?
    };
    if !byte.is_ascii_uppercase() && byte != b'@' {
        mods = mods.unset(Mods::SHIFT);
    }
    if byte.is_ascii_uppercase()
        && event.unshifted_codepoint > 0
        && let Ok(unshifted) = u8::try_from(event.unshifted_codepoint)
    {
        byte = unshifted;
    }
    if mods != Mods::CTRL {
        return None;
    }
    control_byte(byte)
}

/// Traditional control mappings; tab, return and escape ambiguities use CSI u.
fn control_byte(byte: u8) -> Option<u8> {
    Some(match byte {
        b' ' | b'2' | b'@' => 0,
        b'/' | b'7' | b'_' => 31,
        b'0' | b'1' | b'9' => byte,
        b'3' => 27,
        b'4' | b'\\' => 28,
        b'5' | b']' => 29,
        b'6' | b'^' | b'~' => 30,
        b'8' | b'?' => 127,
        b'a'..=b'h' | b'j'..=b'l' | b'n'..=b'z' => byte - b'a' + 1,
        _ => return None,
    })
}

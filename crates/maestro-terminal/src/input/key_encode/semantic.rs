//! Semantic character preparation without a physical key-name table.
use super::super::keyboard::{KeyAction, KeyEvent, Mods};

/// Normalize explicitly shifted letters without inferring a keyboard layout.
pub(super) fn prepare(event: &KeyEvent) -> KeyEvent {
    let mut event = event.clone();
    if event.mods.contains(Mods::SHIFT) {
        if let Some(ch) = char::from_u32(event.unshifted_codepoint).filter(char::is_ascii_uppercase)
        {
            event.unshifted_codepoint = u32::from(ch.to_ascii_lowercase());
        }
        if event.utf8.len() == 1 && event.utf8.as_bytes()[0].is_ascii_lowercase() {
            event.utf8.make_ascii_uppercase();
        }
    }
    event
}

/// Text produced by unmodified or explicitly shifted semantic characters.
pub(super) fn text(event: &KeyEvent) -> Option<&str> {
    if event.action == KeyAction::Release || event.utf8.is_empty() {
        return None;
    }
    if event.mods.empty() {
        return Some(&event.utf8);
    }
    let ch = event.utf8.chars().next()?;
    (event.mods == Mods::SHIFT
        && (ch.is_ascii_alphabetic()
            || "!@#$%^&*()_+{}|:\"<>?~".contains(ch)
            || u32::from(ch) != event.unshifted_codepoint))
        .then_some(event.utf8.as_str())
}

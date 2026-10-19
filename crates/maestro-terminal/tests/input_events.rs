//! Owned keyboard events sent to terminal applications.

use maestro_terminal::input::{
    key::{Key, KeyAction, KeyEvent},
    key_encode::KeyEncoder,
    key_mods::Mods,
};

#[test]
fn key_event_starts_as_an_unidentified_press() {
    let event = KeyEvent::default();
    assert_eq!(event.action, KeyAction::Press);
    assert_eq!(event.key, Key::Unidentified);
    assert!(event.mods.empty());
    assert!(event.consumed_mods.empty());
    assert!(!event.composing);
    assert!(event.utf8.is_empty());
    assert_eq!(event.unshifted_codepoint, 0);
}

#[test]
fn complete_key_event_retains_shifted_text_and_unshifted_codepoint() {
    let mut event = KeyEvent {
        action: KeyAction::Press,
        key: Key::KeyA,
        mods: Mods::SHIFT,
        consumed_mods: Mods::SHIFT,
        unshifted_codepoint: u32::from('a'),
        ..KeyEvent::default()
    };
    event.set_utf8("A");
    assert_eq!(event.key, Key::KeyA);
    assert_eq!(event.action, KeyAction::Press);
    assert_eq!(event.utf8, "A");
    assert_eq!(event.unshifted_codepoint, u32::from('a'));
    assert!(event.effective_mods().empty());
    event.set_utf8("");
    assert_eq!(event.effective_mods(), Mods::SHIFT);
}

#[test]
fn key_events_keep_the_text_supplied_by_the_caller() {
    let mut text = String::from("b");
    let mut event = KeyEvent::default();
    event.set_utf8(&text);
    text.replace_range(.., "c");
    assert_eq!(event.utf8, "b");
    drop(text);
    assert_eq!(event.utf8, "b");
    assert_eq!(KeyEncoder::default().encode(&event), b"b");
    event.set_utf8("é🦀");
    assert_eq!(event.utf8, "é🦀");
    event.set_utf8("");
    assert!(event.utf8.is_empty());
}

#[test]
fn key_event_fields_retain_repeat_release_and_composition() {
    let mut event = KeyEvent {
        action: KeyAction::Repeat,
        key: Key::KeyZ,
        mods: Mods::ALT.union(Mods::SUPER),
        consumed_mods: Mods::ALT,
        composing: true,
        unshifted_codepoint: u32::from('z'),
        ..KeyEvent::default()
    };
    event.set_utf8("test");
    assert_eq!(event.action, KeyAction::Repeat);
    assert_eq!(event.key, Key::KeyZ);
    assert_eq!(event.mods, Mods::ALT.union(Mods::SUPER));
    assert_eq!(event.consumed_mods, Mods::ALT);
    assert!(event.composing);
    assert_eq!(event.utf8, "test");
    assert_eq!(event.unshifted_codepoint, u32::from('z'));
    assert_eq!(event.effective_mods(), Mods::SUPER);
    event.action = KeyAction::Release;
    event.set_utf8("");
    assert_eq!(event.action, KeyAction::Release);
    assert_eq!(event.effective_mods(), event.mods);
}

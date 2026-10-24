//! Enhanced keyboard protocol fields observed at the public byte boundary.
use maestro_terminal::{
    input::{
        key_encode::KeyEncoder,
        key_mods::{Mod, Mods, Side},
        keyboard::{Key, KeyAction, KeyEvent},
    },
    kitty::key::Flags,
};

#[test]
fn protocol_modifier_sequence_values() {
    for (mods, expected) in [
        (Mods::default(), "\x1b[97u"),
        (Mods::SHIFT, "\x1b[97;2u"),
        (Mods::ALT, "\x1b[97;3u"),
        (Mods::CTRL, "\x1b[97;5u"),
        (Mods::ALT.union(Mods::SHIFT), "\x1b[97;4u"),
        (Mods::CTRL.union(Mods::SHIFT), "\x1b[97;6u"),
        (Mods::ALT.union(Mods::CTRL), "\x1b[97;7u"),
        (Mods::ALT.union(Mods::CTRL).union(Mods::SHIFT), "\x1b[97;8u"),
        (Mods::SUPER, "\x1b[97;9u"),
        (Mods::CAPS_LOCK, "\x1b[97;65u"),
        (Mods::NUM_LOCK, "\x1b[97;129u"),
    ] {
        let encoder = KeyEncoder {
            kitty_flags: Flags::REPORT_ALL,
            ..KeyEncoder::default()
        };
        let event = KeyEvent {
            key: Key::KeyA,
            unshifted_codepoint: 97,
            mods,
            ..KeyEvent::default()
        };
        assert_eq!(encoder.encode(&event), expected.as_bytes());
        let sided = KeyEvent {
            mods: mods
                .with_side(Mod::Shift, Side::Right)
                .with_side(Mod::Alt, Side::Right),
            ..event
        };
        assert_eq!(encoder.encode(&sided), expected.as_bytes());
    }
}

#[test]
fn protocol_consumed_modifiers_select_text_but_preserve_wire_modifiers() {
    let event = KeyEvent {
        key: Key::KeyA,
        unshifted_codepoint: 97,
        utf8: "A".into(),
        mods: Mods::SHIFT,
        consumed_mods: Mods::SHIFT,
        ..KeyEvent::default()
    };
    let mut encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(encoder.encode(&event), b"A");
    encoder.kitty_flags = Flags::TRUE;
    assert_eq!(encoder.encode(&event), b"\x1b[97:65;2;65u");
}

#[test]
fn protocol_repeat_release_and_special_press_fields() {
    let mut encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    let mut event = KeyEvent {
        key: Key::ArrowUp,
        action: KeyAction::Release,
        ..KeyEvent::default()
    };
    assert!(encoder.encode(&event).is_empty());
    encoder.kitty_flags = Flags::REPORT_EVENTS;
    assert_eq!(encoder.encode(&event), b"\x1b[1;1:3A");
    event.action = KeyAction::Repeat;
    assert_eq!(encoder.encode(&event), b"\x1b[1;1:2A");
    event.action = KeyAction::Press;
    assert_eq!(encoder.encode(&event), b"\x1b[1;1:1A");
    event.key = Key::KeyA;
    event.unshifted_codepoint = 97;
    event.action = KeyAction::Repeat;
    assert_eq!(encoder.encode(&event), b"\x1b[97;1:2u");
}

#[test]
fn protocol_missing_key_and_multicodepoint_text() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert!(encoder.encode(&KeyEvent::default()).is_empty());
    let event = KeyEvent {
        key: Key::Semicolon,
        unshifted_codepoint: 1095,
        utf8: "Чж".into(),
        mods: Mods::SHIFT,
        ..KeyEvent::default()
    };
    assert_eq!(encoder.encode(&event), b"\x1b[1095:1063;2;1063:1078u");
}

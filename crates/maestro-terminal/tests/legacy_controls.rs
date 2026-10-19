//! Control bindings observed at the encoded-byte boundary.

use maestro_terminal::input::{
    key::{Key, KeyAction, KeyEvent},
    key_encode::KeyEncoder,
    key_mods::{Mod, Mods, Side},
};

#[test]
fn control_c_accepts_sides_locks_and_alt() {
    for (mods, text, expected) in [
        (Mods::CTRL, "c", b"\x03".as_slice()),
        (Mods::CTRL.with_side(Mod::Ctrl, Side::Right), "c", b"\x03"),
        (Mods::CTRL.union(Mods::ALT), "c", b"\x1b\x03"),
        (Mods::CTRL.union(Mods::CAPS_LOCK), "C", b"\x03"),
    ] {
        let event = KeyEvent {
            utf8: text.into(),
            mods,
            unshifted_codepoint: u32::from('c'),
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), expected);
    }
}

#[test]
fn control_sequences_require_control_and_preserve_shift() {
    for (mods, text, base, expected) in [
        (Mods::default(), "c", 'c', b"c".as_slice()),
        (Mods::SHIFT, "C", 'c', b"C"),
        (Mods::CTRL.union(Mods::SHIFT), "C", 'c', b"\x1b[99;6u"),
        (Mods::CTRL.union(Mods::SHIFT), "_", '-', b"\x1f"),
    ] {
        let event = KeyEvent {
            utf8: text.into(),
            mods,
            unshifted_codepoint: u32::from(base),
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), expected);
    }
}

#[test]
fn cyrillic_control_uses_physical_key_unless_shifted() {
    for (mods, expected) in [
        (Mods::CTRL, b"\x03".as_slice()),
        (Mods::CTRL.union(Mods::SHIFT), b"\x1b[1089;6u"),
        (Mods::CTRL.union(Mods::ALT), b"\x1b\x03"),
        (Mods::CTRL.with_side(Mod::Ctrl, Side::Right), b"\x03"),
    ] {
        let event = KeyEvent {
            key: Key::KeyC,
            utf8: "с".into(),
            mods,
            unshifted_codepoint: u32::from('с'),
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), expected);
    }
}

#[test]
fn traditional_control_bytes_cover_digits_and_punctuation() {
    for (text, byte) in [
        (" ", 0),
        ("/", 31),
        ("0", 48),
        ("1", 49),
        ("2", 0),
        ("3", 27),
        ("4", 28),
        ("5", 29),
        ("6", 30),
        ("7", 31),
        ("8", 127),
        ("9", 57),
        ("?", 127),
        ("@", 0),
        ("\\", 28),
        ("]", 29),
        ("^", 30),
        ("_", 31),
        ("~", 30),
    ] {
        let event = KeyEvent {
            utf8: text.into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), [byte], "{text}");
    }
}

#[test]
fn traditional_control_letters_keep_ambiguous_keys_distinct() {
    let bytes: [&[u8]; 26] = [
        b"\x01",
        b"\x02",
        b"\x03",
        b"\x04",
        b"\x05",
        b"\x06",
        b"\x07",
        b"\x08",
        b"\x1b[105;5u",
        b"\x0a",
        b"\x0b",
        b"\x0c",
        b"\x1b[109;5u",
        b"\x0e",
        b"\x0f",
        b"\x10",
        b"\x11",
        b"\x12",
        b"\x13",
        b"\x14",
        b"\x15",
        b"\x16",
        b"\x17",
        b"\x18",
        b"\x19",
        b"\x1a",
    ];
    for (letter, expected) in ('a'..='z').zip(bytes) {
        let event = KeyEvent {
            utf8: letter.into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), expected);
    }
}

#[test]
fn legacy_drops_release_and_composition_but_repeats_text() {
    let mut event = KeyEvent {
        key: Key::KeyA,
        utf8: "a".into(),
        ..KeyEvent::default()
    };
    let encoder = KeyEncoder::default();
    event.action = KeyAction::Repeat;
    assert_eq!(encoder.encode(&event), b"a");
    event.action = KeyAction::Release;
    assert!(encoder.encode(&event).is_empty());
    event.action = KeyAction::Press;
    event.composing = true;
    assert!(encoder.encode(&event).is_empty());
    event.composing = false;
    event.utf8.clear();
    assert!(encoder.encode(&event).is_empty());
}

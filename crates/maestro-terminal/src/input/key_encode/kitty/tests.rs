//! Prepared keyboard sequence serialization at its private owner.
use super::{Entry, Sequence};

/// Construct the smallest sequence with supplied wire fields.
fn sequence(mods: u16, event: u8, text: &str, final_char: char) -> Sequence<'_> {
    Sequence {
        entry: Entry {
            code: 127,
            final_char,
            modifier: false,
        },
        semantic: false,
        mods,
        event,
        alternates: [None; 2],
        text,
    }
}

#[test]
fn sequence_backspace() {
    for (mods, event, expected) in [
        (1, 0, "\x1b[127u"),
        (1, 3, "\x1b[127;1:3u"),
        (2, 0, "\x1b[127;2u"),
    ] {
        assert_eq!(sequence(mods, event, "", 'u').encode(), expected.as_bytes());
    }
}

#[test]
fn sequence_text() {
    for (mods, event, expected) in [
        (1, 0, "\x1b[127;;65u"),
        (1, 3, "\x1b[127;1:3;65u"),
        (2, 0, "\x1b[127;2;65u"),
    ] {
        assert_eq!(
            sequence(mods, event, "A", 'u').encode(),
            expected.as_bytes()
        );
    }
}

#[test]
fn sequence_text_with_control_characters() {
    for (text, expected) in [
        ("\n", "\x1b[127u"),
        ("A\n", "\x1b[127;;65u"),
        ("A\nβ", "\x1b[127;;65:946u"),
    ] {
        assert_eq!(sequence(1, 0, text, 'u').encode(), expected.as_bytes());
    }
}

#[test]
fn sequence_special_no_mods() {
    assert_eq!(sequence(1, 0, "", 'A').encode(), b"\x1b[A");
}

#[test]
fn sequence_special_mods_only() {
    assert_eq!(sequence(2, 0, "", 'A').encode(), b"\x1b[1;2A");
}

#[test]
fn sequence_special_mods_and_event() {
    assert_eq!(sequence(2, 3, "", 'A').encode(), b"\x1b[1;2:3A");
}

#[test]
fn semantic_extended_modifiers_do_not_collapse_to_plain_text() {
    use crate::input::{
        key::KeyEvent,
        key_encode::{KeyEncoder, KeyEncoding},
        key_mods::Mods,
    };
    use crate::kitty::key::Flags;
    let encoder = KeyEncoder {
        encoding: KeyEncoding::Semantic,
        kitty_flags: Flags::from_int(1).unwrap(),
        ..KeyEncoder::default()
    };
    for (bits, expected) in [
        (1 << 10, b"\x1b[97;17u".as_slice()),
        (1 << 11, b"\x1b[97;33u"),
    ] {
        let event = KeyEvent {
            mods: Mods::from_int(bits),
            unshifted_codepoint: 97,
            utf8: "a".into(),
            ..KeyEvent::default()
        };
        assert_eq!(encoder.encode(&event), expected);
    }
}

#[test]
fn semantic_legacy_alt_enter_keeps_escape_prefix() {
    use crate::input::{
        key::{Key, KeyEvent},
        key_encode::{KeyEncoder, KeyEncoding},
        key_mods::Mods,
    };
    let encoder = KeyEncoder {
        encoding: KeyEncoding::Semantic,
        ..KeyEncoder::default()
    };
    for (mods, expected) in [
        (Mods::ALT, b"\x1b\r".as_slice()),
        (Mods::ALT.union(Mods::SHIFT), b"\x1b\r"),
        (Mods::ALT.union(Mods::CTRL), b"\x1b\r"),
        (Mods::ALT.union(Mods::HYPER), b"\x1b\r"),
        (Mods::SHIFT, b"\r"),
        (Mods::CTRL, b"\r"),
    ] {
        assert_eq!(
            encoder.encode(&KeyEvent {
                key: Key::Enter,
                mods,
                ..KeyEvent::default()
            }),
            expected
        );
    }
}

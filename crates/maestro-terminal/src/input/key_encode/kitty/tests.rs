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

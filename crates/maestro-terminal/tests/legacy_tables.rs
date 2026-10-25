//! Function-key, cursor and keypad mode tables at the byte boundary.

use maestro_terminal::{
    input::{
        key::{Key, KeyEvent},
        key_encode::KeyEncoder,
        key_mods::Mods,
    },
    modes::Mode,
};

#[test]
fn normal_function_keys_use_their_protocol_sequences() {
    for (key, expected) in [
        (Key::ArrowUp, "\x1b[A"),
        (Key::ArrowDown, "\x1b[B"),
        (Key::ArrowRight, "\x1b[C"),
        (Key::ArrowLeft, "\x1b[D"),
        (Key::Home, "\x1b[H"),
        (Key::End, "\x1b[F"),
        (Key::Insert, "\x1b[2~"),
        (Key::Delete, "\x1b[3~"),
        (Key::PageUp, "\x1b[5~"),
        (Key::PageDown, "\x1b[6~"),
        (Key::F1, "\x1bOP"),
        (Key::F2, "\x1bOQ"),
        (Key::F3, "\x1bOR"),
        (Key::F4, "\x1bOS"),
        (Key::F5, "\x1b[15~"),
        (Key::F6, "\x1b[17~"),
        (Key::F7, "\x1b[18~"),
        (Key::F8, "\x1b[19~"),
        (Key::F9, "\x1b[20~"),
        (Key::F10, "\x1b[21~"),
        (Key::F11, "\x1b[23~"),
        (Key::F12, "\x1b[24~"),
        (Key::NumpadUp, "\x1b[A"),
        (Key::NumpadDown, "\x1b[B"),
        (Key::NumpadRight, "\x1b[C"),
        (Key::NumpadLeft, "\x1b[D"),
        (Key::NumpadBegin, "\x1b[E"),
        (Key::NumpadHome, "\x1b[H"),
        (Key::NumpadEnd, "\x1b[F"),
        (Key::NumpadInsert, "\x1b[2~"),
        (Key::NumpadDelete, "\x1b[3~"),
        (Key::NumpadPageUp, "\x1b[5~"),
        (Key::NumpadPageDown, "\x1b[6~"),
    ] {
        let event = KeyEvent {
            key,
            mods: Mods::NUM_LOCK,
            ..KeyEvent::default()
        };
        assert_eq!(
            KeyEncoder::default().encode(&event),
            expected.as_bytes(),
            "{key:?}"
        );
    }
}

#[test]
fn application_cursors_and_keypad_use_distinct_suffixes() {
    let mut encoder = KeyEncoder::default();
    encoder.modes.set(Mode::CursorKeys, true);
    encoder.modes.set(Mode::KeypadKeys, true);
    for (key, expected) in [
        (Key::ArrowUp, "\x1bOA"),
        (Key::ArrowDown, "\x1bOB"),
        (Key::ArrowRight, "\x1bOC"),
        (Key::ArrowLeft, "\x1bOD"),
        (Key::Home, "\x1bOH"),
        (Key::End, "\x1bOF"),
        (Key::Numpad0, "\x1bOp"),
        (Key::Numpad1, "\x1bOq"),
        (Key::Numpad2, "\x1bOr"),
        (Key::Numpad3, "\x1bOs"),
        (Key::Numpad4, "\x1bOt"),
        (Key::Numpad5, "\x1bOu"),
        (Key::Numpad6, "\x1bOv"),
        (Key::Numpad7, "\x1bOw"),
        (Key::Numpad8, "\x1bOx"),
        (Key::Numpad9, "\x1bOy"),
        (Key::NumpadDecimal, "\x1bOn"),
        (Key::NumpadDivide, "\x1bOo"),
        (Key::NumpadMultiply, "\x1bOj"),
        (Key::NumpadSubtract, "\x1bOm"),
        (Key::NumpadAdd, "\x1bOk"),
        (Key::NumpadEnter, "\x1bOM"),
    ] {
        assert_eq!(
            encoder.encode(&KeyEvent {
                key,
                ..KeyEvent::default()
            }),
            expected.as_bytes()
        );
    }
    let event = KeyEvent {
        key: Key::Numpad1,
        mods: Mods::CTRL,
        ..KeyEvent::default()
    };
    assert_eq!(encoder.encode(&event), b"\x1bO5q");
    encoder.modes.set(Mode::IgnoreKeypadWithNumlock, true);
    assert_eq!(encoder.encode(&event), b"1");
}

#[test]
fn all_binding_modifier_combinations_have_the_protocol_order() {
    let modifiers = [1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];
    for (bits, code) in modifiers.into_iter().zip(2..=16) {
        let event = KeyEvent {
            key: Key::ArrowUp,
            mods: Mods::from_int(bits),
            ..KeyEvent::default()
        };
        assert_eq!(
            KeyEncoder::default().encode(&event),
            format!("\x1b[1;{code}A").as_bytes()
        );
    }
}

#[test]
fn backspace_special_modifier_entries_precede_backarrow_default() {
    let cases: [(u16, &[u8]); 16] = [
        (0, b"\x7f"),
        (1, b"\x7f"),
        (4, b"\x1b\x7f"),
        (5, b"\x1b\x7f"),
        (2, b"\x08"),
        (3, b"\x08"),
        (6, b"\x1b\x08"),
        (7, b"\x7f"),
        (8, b"\x7f"),
        (9, b"\x7f"),
        (12, b"\x1b\x7f"),
        (13, b"\x1b\x7f"),
        (10, b"\x08"),
        (11, b"\x08"),
        (14, b"\x1b\x08"),
        (15, b"\x1b\x08"),
    ];
    for (bits, expected) in cases {
        let event = KeyEvent {
            key: Key::Backspace,
            mods: Mods::from_int(bits),
            ..KeyEvent::default()
        };
        assert_eq!(KeyEncoder::default().encode(&event), expected);
    }
}

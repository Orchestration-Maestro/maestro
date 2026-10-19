//! Functional key wire codes at the public encoder boundary.
use maestro_terminal::{
    input::{
        key::{Key, KeyEvent},
        key_encode::KeyEncoder,
    },
    kitty::key::Flags,
};

#[test]
fn protocol_functional_key_table() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::REPORT_ALL,
        ..KeyEncoder::default()
    };
    for &(key, expected) in FUNCTIONAL_KEYS {
        assert_eq!(
            encoder.encode(&KeyEvent {
                key,
                ..KeyEvent::default()
            }),
            expected.as_bytes(),
            "{key:?}"
        );
    }
}

/// Expected bytes for the protocol functional keys.
const FUNCTIONAL_KEYS: &[(Key, &str)] = &[
    (Key::Escape, "\x1b[27u"),
    (Key::Enter, "\x1b[13u"),
    (Key::Tab, "\x1b[9u"),
    (Key::Backspace, "\x1b[127u"),
    (Key::Insert, "\x1b[2~"),
    (Key::Delete, "\x1b[3~"),
    (Key::ArrowLeft, "\x1b[D"),
    (Key::ArrowRight, "\x1b[C"),
    (Key::ArrowUp, "\x1b[A"),
    (Key::ArrowDown, "\x1b[B"),
    (Key::PageUp, "\x1b[5~"),
    (Key::PageDown, "\x1b[6~"),
    (Key::Home, "\x1b[H"),
    (Key::End, "\x1b[F"),
    (Key::CapsLock, "\x1b[57358u"),
    (Key::ScrollLock, "\x1b[57359u"),
    (Key::NumLock, "\x1b[57360u"),
    (Key::PrintScreen, "\x1b[57361u"),
    (Key::Pause, "\x1b[57362u"),
    (Key::F1, "\x1b[P"),
    (Key::F2, "\x1b[Q"),
    (Key::F3, "\x1b[13~"),
    (Key::F4, "\x1b[S"),
    (Key::F5, "\x1b[15~"),
    (Key::F6, "\x1b[17~"),
    (Key::F7, "\x1b[18~"),
    (Key::F8, "\x1b[19~"),
    (Key::F9, "\x1b[20~"),
    (Key::F10, "\x1b[21~"),
    (Key::F11, "\x1b[23~"),
    (Key::F12, "\x1b[24~"),
    (Key::F13, "\x1b[57376u"),
    (Key::F14, "\x1b[57377u"),
    (Key::F15, "\x1b[57378u"),
    (Key::F16, "\x1b[57379u"),
    (Key::F17, "\x1b[57380u"),
    (Key::F18, "\x1b[57381u"),
    (Key::F19, "\x1b[57382u"),
    (Key::F20, "\x1b[57383u"),
    (Key::F21, "\x1b[57384u"),
    (Key::F22, "\x1b[57385u"),
    (Key::F23, "\x1b[57386u"),
    (Key::F24, "\x1b[57387u"),
    (Key::F25, "\x1b[57388u"),
    (Key::Numpad0, "\x1b[57399u"),
    (Key::Numpad1, "\x1b[57400u"),
    (Key::Numpad2, "\x1b[57401u"),
    (Key::Numpad3, "\x1b[57402u"),
    (Key::Numpad4, "\x1b[57403u"),
    (Key::Numpad5, "\x1b[57404u"),
    (Key::Numpad6, "\x1b[57405u"),
    (Key::Numpad7, "\x1b[57406u"),
    (Key::Numpad8, "\x1b[57407u"),
    (Key::Numpad9, "\x1b[57408u"),
    (Key::NumpadDecimal, "\x1b[57409u"),
    (Key::NumpadDivide, "\x1b[57410u"),
    (Key::NumpadMultiply, "\x1b[57411u"),
    (Key::NumpadSubtract, "\x1b[57412u"),
    (Key::NumpadAdd, "\x1b[57413u"),
    (Key::NumpadEnter, "\x1b[57414u"),
    (Key::NumpadEqual, "\x1b[57415u"),
    (Key::NumpadSeparator, "\x1b[57416u"),
    (Key::NumpadLeft, "\x1b[57417u"),
    (Key::NumpadRight, "\x1b[57418u"),
    (Key::NumpadUp, "\x1b[57419u"),
    (Key::NumpadDown, "\x1b[57420u"),
    (Key::NumpadPageUp, "\x1b[57421u"),
    (Key::NumpadPageDown, "\x1b[57422u"),
    (Key::NumpadHome, "\x1b[57423u"),
    (Key::NumpadEnd, "\x1b[57424u"),
    (Key::NumpadInsert, "\x1b[57425u"),
    (Key::NumpadDelete, "\x1b[57426u"),
    (Key::NumpadBegin, "\x1b[57427u"),
    (Key::ShiftLeft, "\x1b[57441u"),
    (Key::ShiftRight, "\x1b[57447u"),
    (Key::ControlLeft, "\x1b[57442u"),
    (Key::ControlRight, "\x1b[57448u"),
    (Key::MetaLeft, "\x1b[57444u"),
    (Key::MetaRight, "\x1b[57450u"),
    (Key::AltLeft, "\x1b[57443u"),
    (Key::AltRight, "\x1b[57449u"),
];

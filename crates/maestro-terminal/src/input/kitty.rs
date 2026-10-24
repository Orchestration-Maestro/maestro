//! Functional key codes in the enhanced keyboard protocol.
use super::keyboard::Key;

/// A functional key's wire code and sequence terminator.
#[derive(Clone, Copy)]
pub(super) struct Entry {
    /// Primary protocol code.
    pub code: u32,
    /// Sequence terminator.
    pub final_char: char,
    /// Whether only report-all mode permits this key.
    pub modifier: bool,
}

/// Functional key table, ordered by protocol key definitions.
const ENTRIES: &[(Key, u32, char, bool)] = &[
    (Key::Escape, 27, 'u', false),
    (Key::Enter, 13, 'u', false),
    (Key::Tab, 9, 'u', false),
    (Key::Backspace, 127, 'u', false),
    (Key::Insert, 2, '~', false),
    (Key::Delete, 3, '~', false),
    (Key::ArrowLeft, 1, 'D', false),
    (Key::ArrowRight, 1, 'C', false),
    (Key::ArrowUp, 1, 'A', false),
    (Key::ArrowDown, 1, 'B', false),
    (Key::PageUp, 5, '~', false),
    (Key::PageDown, 6, '~', false),
    (Key::Home, 1, 'H', false),
    (Key::End, 1, 'F', false),
    (Key::CapsLock, 57358, 'u', true),
    (Key::ScrollLock, 57359, 'u', false),
    (Key::NumLock, 57360, 'u', true),
    (Key::PrintScreen, 57361, 'u', false),
    (Key::Pause, 57362, 'u', false),
    (Key::F1, 1, 'P', false),
    (Key::F2, 1, 'Q', false),
    (Key::F3, 13, '~', false),
    (Key::F4, 1, 'S', false),
    (Key::F5, 15, '~', false),
    (Key::F6, 17, '~', false),
    (Key::F7, 18, '~', false),
    (Key::F8, 19, '~', false),
    (Key::F9, 20, '~', false),
    (Key::F10, 21, '~', false),
    (Key::F11, 23, '~', false),
    (Key::F12, 24, '~', false),
    (Key::F13, 57376, 'u', false),
    (Key::F14, 57377, 'u', false),
    (Key::F15, 57378, 'u', false),
    (Key::F16, 57379, 'u', false),
    (Key::F17, 57380, 'u', false),
    (Key::F18, 57381, 'u', false),
    (Key::F19, 57382, 'u', false),
    (Key::F20, 57383, 'u', false),
    (Key::F21, 57384, 'u', false),
    (Key::F22, 57385, 'u', false),
    (Key::F23, 57386, 'u', false),
    (Key::F24, 57387, 'u', false),
    (Key::F25, 57388, 'u', false),
    (Key::Numpad0, 57399, 'u', false),
    (Key::Numpad1, 57400, 'u', false),
    (Key::Numpad2, 57401, 'u', false),
    (Key::Numpad3, 57402, 'u', false),
    (Key::Numpad4, 57403, 'u', false),
    (Key::Numpad5, 57404, 'u', false),
    (Key::Numpad6, 57405, 'u', false),
    (Key::Numpad7, 57406, 'u', false),
    (Key::Numpad8, 57407, 'u', false),
    (Key::Numpad9, 57408, 'u', false),
    (Key::NumpadDecimal, 57409, 'u', false),
    (Key::NumpadDivide, 57410, 'u', false),
    (Key::NumpadMultiply, 57411, 'u', false),
    (Key::NumpadSubtract, 57412, 'u', false),
    (Key::NumpadAdd, 57413, 'u', false),
    (Key::NumpadEnter, 57414, 'u', false),
    (Key::NumpadEqual, 57415, 'u', false),
    (Key::NumpadSeparator, 57416, 'u', false),
    (Key::NumpadLeft, 57417, 'u', false),
    (Key::NumpadRight, 57418, 'u', false),
    (Key::NumpadUp, 57419, 'u', false),
    (Key::NumpadDown, 57420, 'u', false),
    (Key::NumpadPageUp, 57421, 'u', false),
    (Key::NumpadPageDown, 57422, 'u', false),
    (Key::NumpadHome, 57423, 'u', false),
    (Key::NumpadEnd, 57424, 'u', false),
    (Key::NumpadInsert, 57425, 'u', false),
    (Key::NumpadDelete, 57426, 'u', false),
    (Key::NumpadBegin, 57427, 'u', false),
    (Key::ShiftLeft, 57441, 'u', true),
    (Key::ShiftRight, 57447, 'u', true),
    (Key::ControlLeft, 57442, 'u', true),
    (Key::ControlRight, 57448, 'u', true),
    (Key::MetaLeft, 57444, 'u', true),
    (Key::MetaRight, 57450, 'u', true),
    (Key::AltLeft, 57443, 'u', true),
    (Key::AltRight, 57449, 'u', true),
];

/// Find a functional key without mapping printable physical keys.
pub(super) fn entry(key: Key) -> Option<Entry> {
    ENTRIES
        .iter()
        .find(|(candidate, ..)| *candidate == key)
        .map(|&(_, code, final_char, modifier)| Entry {
            code,
            final_char,
            modifier,
        })
}

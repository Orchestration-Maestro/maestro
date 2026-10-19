//! Legacy keyboard bytes for pane applications.
use maestro_terminal::input::{
    key::{Key, KeyEvent},
    key_encode::{KeyEncoder, OptionAsAlt},
    key_mods::{Mod, Mods, Side},
};
use maestro_terminal::modes::{Mode, ModeState};

#[test]
fn legacy_backspace_with_utf8_dead_key_state() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Backspace,
            utf8: "A".into(),
            unshifted_codepoint: 0x0D,
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
fn legacy_enter_with_utf8_dead_key_state() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Enter,
            utf8: "A".into(),
            unshifted_codepoint: 0x0D,
            ..KeyEvent::default()
        }),
        "A".as_bytes()
    );
}

#[test]
fn legacy_esc_with_utf8_dead_key_state() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Escape,
            utf8: "A".into(),
            unshifted_codepoint: 0x0D,
            ..KeyEvent::default()
        }),
        "A".as_bytes()
    );
}

#[test]
fn legacy_ctrl_shift_minus_underscore_on_us() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Minus,
            utf8: "_".into(),
            mods: Mods::CTRL.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x1F".as_bytes()
    );
}

#[test]
fn legacy_ctrl_alt_c() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyC,
            utf8: "c".into(),
            mods: Mods::CTRL.union(Mods::ALT),
            ..KeyEvent::default()
        }),
        "\x1b\x03".as_bytes()
    );
}

#[test]
fn legacy_alt_c() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::AltEscPrefix, true)]),

            macos_option_as_alt: OptionAsAlt::True,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyC,
            utf8: "c".into(),
            mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "\x1Bc".as_bytes()
    );
}

#[test]
fn legacy_alt_e_only_unshifted() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::AltEscPrefix, true)]),

            macos_option_as_alt: OptionAsAlt::True,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyE,
            unshifted_codepoint: u32::from('e'),
            mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "\x1Be".as_bytes()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn legacy_alt_x_macos() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::AltEscPrefix, true)]),

            macos_option_as_alt: OptionAsAlt::True,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyC,
            utf8: "≈".into(),
            unshifted_codepoint: u32::from('c'),
            mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "\x1Bc".as_bytes()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn legacy_shift_alt_macos() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::AltEscPrefix, true)]),

            macos_option_as_alt: OptionAsAlt::True,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Period,
            utf8: ">".into(),
            unshifted_codepoint: u32::from('.'),
            mods: Mods::ALT.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x1B>".as_bytes()
    );
}

#[test]
fn legacy_alt_cyrillic_text() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::AltEscPrefix, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyF,
            utf8: "ф".into(),
            mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "ф".as_bytes()
    );
}

#[test]
fn legacy_ctrl_c() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyC,
            utf8: "c".into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x03".as_bytes()
    );
}

#[test]
fn legacy_ctrl_space() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Space,
            utf8: " ".into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x00".as_bytes()
    );
}

#[test]
fn legacy_ctrl_shift_backspace() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Backspace,
            mods: Mods::CTRL.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x08".as_bytes()
    );
}

#[test]
fn legacy_backspace_decbkm_reset() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, false)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            ..KeyEvent::default()
        }),
        "\x7f".as_bytes()
    );
}

#[test]
fn legacy_backspace_decbkm_reset_with_ctrl() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, false)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x08".as_bytes()
    );
}

#[test]
fn legacy_backspace_decbkm_set() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            ..KeyEvent::default()
        }),
        "\x08".as_bytes()
    );
}

#[test]
fn legacy_backspace_decbkm_set_with_ctrl() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x7f".as_bytes()
    );
}

#[test]
fn legacy_ctrl_shift_char_with_modify_other_state_2() {
    assert_eq!(
        KeyEncoder {
            modify_other_keys_state_2: true,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyH,
            utf8: "H".into(),
            mods: Mods::CTRL.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x1b[27;6;72~".as_bytes()
    );
}

#[test]
fn legacy_ctrl_shift_char_with_modify_other_state_2_and_consumed_mods() {
    assert_eq!(
        KeyEncoder {
            modify_other_keys_state_2: true,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::KeyH,
            utf8: "H".into(),
            mods: Mods::CTRL.union(Mods::SHIFT),
            consumed_mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[27;6;72~".as_bytes()
    );
}

#[test]
fn legacy_alt_digit_with_modify_other_state_2() {
    assert_eq!(
        KeyEncoder {
            modify_other_keys_state_2: true,
            macos_option_as_alt: OptionAsAlt::True,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Digit8,
            utf8: "8".into(),
            mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "\x1b[27;3;56~".as_bytes()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn legacy_alt_digit_with_modify_other_state_2_and_macos_option_as_alt_false() {
    assert_eq!(
        KeyEncoder {
            modify_other_keys_state_2: true,
            macos_option_as_alt: OptionAsAlt::False,
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Digit8,
            utf8: "[".into(),
            mods: Mods::ALT,
            consumed_mods: Mods::ALT,
            ..KeyEvent::default()
        }),
        "[".as_bytes()
    );
}

#[test]
fn legacy_fixterm_awkward_letters() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyI,
            utf8: "i".into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[105;5u".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyM,
            utf8: "m".into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[109;5u".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::BracketLeft,
            utf8: "[".into(),
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[91;5u".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Digit2,
            utf8: "@".into(),
            unshifted_codepoint: u32::from('2'),
            mods: Mods::CTRL.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x1b[64;5u".as_bytes()
    );
}

#[test]
fn legacy_ctrl_shift_letter_ascii() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyM,
            utf8: "M".into(),
            unshifted_codepoint: u32::from('m'),
            mods: Mods::CTRL.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "\x1b[109;6u".as_bytes()
    );
}

#[test]
fn legacy_shift_function_key_should_use_all_mods() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::ArrowUp,
            mods: Mods::SHIFT,
            consumed_mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[1;2A".as_bytes()
    );
}

#[test]
fn legacy_keypad_enter() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::NumpadEnter,
            ..KeyEvent::default()
        }),
        "\r".as_bytes()
    );
}

#[test]
fn legacy_keypad_1() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Numpad1,
            utf8: "1".into(),
            ..KeyEvent::default()
        }),
        "1".as_bytes()
    );
}

#[test]
fn legacy_keypad_1_with_application_keypad() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::KeypadKeys, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Numpad1,
            utf8: "1".into(),
            ..KeyEvent::default()
        }),
        "\x1bOq".as_bytes()
    );
}

#[test]
fn legacy_keypad_1_with_application_keypad_and_numlock() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::KeypadKeys, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Numpad1,
            utf8: "1".into(),
            mods: Mods::NUM_LOCK,
            ..KeyEvent::default()
        }),
        "\x1bOq".as_bytes()
    );
}

#[test]
fn legacy_keypad_1_with_application_keypad_and_numlock_ignore() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[
                (Mode::KeypadKeys, true),
                (Mode::IgnoreKeypadWithNumlock, true)
            ]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Numpad1,
            utf8: "1".into(),
            ..KeyEvent::default()
        }),
        "1".as_bytes()
    );
}

#[test]
fn legacy_f1() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::F1,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[1;5P".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::F2,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[1;5Q".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::F3,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[13;5~".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::F4,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[1;5S".as_bytes()
    );
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::F5,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[15;5~".as_bytes()
    );
}

#[test]
fn legacy_left_shift_tab() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Tab,
            mods: Mods::SHIFT.with_side(Mod::Shift, Side::Left),
            ..KeyEvent::default()
        }),
        "\x1b[Z".as_bytes()
    );
}

#[test]
fn legacy_right_shift_tab() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::Tab,
            mods: Mods::SHIFT.with_side(Mod::Shift, Side::Right),
            ..KeyEvent::default()
        }),
        "\x1b[Z".as_bytes()
    );
}

#[test]
fn legacy_hu_layout_ctrl_sends_proper_codepoint() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::BracketLeft,
            utf8: "ő".into(),
            unshifted_codepoint: 337,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[337;5u".as_bytes()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn legacy_super_only_on_macos_with_text() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyB,
            utf8: "b".into(),
            mods: Mods::SUPER,
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn legacy_super_and_other_mods_on_macos_with_text() {
    assert_eq!(
        KeyEncoder::default().encode(&KeyEvent {
            key: Key::KeyB,
            utf8: "B".into(),
            mods: Mods::SUPER.union(Mods::SHIFT),
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
fn legacy_backspace_with_del_utf8_decbkm_reset() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, false)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            utf8: "\x7f".into(),
            unshifted_codepoint: 0x08,
            ..KeyEvent::default()
        }),
        "\x7F".as_bytes()
    );
}

#[test]
fn legacy_backspace_with_del_utf8_decbkm_set() {
    assert_eq!(
        KeyEncoder {
            modes: mode_overrides(&[(Mode::BackarrowKeyMode, true)]),
            ..KeyEncoder::default()
        }
        .encode(&KeyEvent {
            key: Key::Backspace,
            utf8: "\x7f".into(),
            unshifted_codepoint: 0x08,
            ..KeyEvent::default()
        }),
        "\x08".as_bytes()
    );
}

/// Construct an encoder snapshot with selected input modes changed.
fn mode_overrides(changes: &[(Mode, bool)]) -> ModeState {
    let mut modes = KeyEncoder::default().modes;
    for &(mode, value) in changes {
        modes.set(mode, value);
    }
    modes
}

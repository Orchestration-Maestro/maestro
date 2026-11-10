//! Keyboard protocol bytes for pane applications.
#[cfg(target_os = "macos")]
use maestro_terminal::input::key_encode::OptionAsAlt;
use maestro_terminal::{
    input::{
        key_encode::KeyEncoder,
        key_mods::Mods,
        keyboard::{Key, KeyAction, KeyEvent},
    },
    kitty::key::Flags,
    modes::Mode,
};
#[test]
fn protocol_plain_text() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyA,
            utf8: "abcd".into(),
            ..KeyEvent::default()
        }),
        "abcd".as_bytes()
    );
}

#[test]
fn protocol_repeat_with_just_disambiguate() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyA,
            action: KeyAction::Repeat,
            utf8: "a".into(),
            ..KeyEvent::default()
        }),
        "a".as_bytes()
    );
}

#[test]
fn protocol_enter_backspace_tab() {
    for (key, code, plain) in [
        (Key::Enter, 13, b'\r'),
        (Key::Backspace, 127, 0x7f),
        (Key::Tab, 9, b'\t'),
    ] {
        for backarrow in [false, true] {
            let mut encoder = KeyEncoder {
                kitty_flags: Flags::DISAMBIGUATE,
                ..KeyEncoder::default()
            };
            encoder.modes.set(Mode::BackarrowKeyMode, backarrow);
            let mut event = KeyEvent {
                key,
                ..KeyEvent::default()
            };
            assert_eq!(encoder.encode(&event), [plain]);
            event.action = KeyAction::Release;
            encoder.kitty_flags = Flags::from_int(3).unwrap();
            assert!(encoder.encode(&event).is_empty());
            encoder.kitty_flags = Flags::from_int(11).unwrap();
            assert_eq!(
                encoder.encode(&event),
                format!("\x1b[{code};1:3u").as_bytes()
            );
        }
    }
}

#[test]
fn protocol_shift_backspace_emits_csi_u() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Backspace,
            mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[127;2u".as_bytes()
    );
}

#[test]
fn protocol_shift_enter_emits_csi_u() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Enter,
            mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[13;2u".as_bytes()
    );
}

#[test]
fn protocol_shift_tab_emits_csi_u() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Tab,
            mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[9;2u".as_bytes()
    );
}

#[test]
fn protocol_enter_with_all_flags() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Enter,
            ..KeyEvent::default()
        }),
        "\x1b[13u".as_bytes()
    );
}

#[test]
fn protocol_ctrl_with_all_flags() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ControlLeft,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[57442;5u".as_bytes()
    );
}

#[test]
fn protocol_ctrl_release_with_ctrl_mod_set() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ControlLeft,
            action: KeyAction::Release,
            mods: Mods::CTRL,
            ..KeyEvent::default()
        }),
        "\x1b[57442;5:3u".as_bytes()
    );
}

#[test]
fn protocol_delete() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Delete,
            utf8: "\x7F".into(),
            ..KeyEvent::default()
        }),
        "\x1b[3~".as_bytes()
    );
}

#[test]
fn protocol_composing_with_no_modifier() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyA,
            mods: Mods::SHIFT,
            composing: true,
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
fn protocol_composing_with_modifier() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(9).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ShiftLeft,
            mods: Mods::SHIFT,
            composing: true,
            ..KeyEvent::default()
        }),
        "\x1b[57441;2u".as_bytes()
    );
}

#[test]
fn protocol_composed_text_with_report_all() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Unidentified,
            utf8: "û".into(),
            ..KeyEvent::default()
        }),
        "û".as_bytes()
    );
}

#[test]
fn protocol_shift_a_on_us_keyboard() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(5).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyA,
            mods: Mods::SHIFT,
            utf8: "A".into(),
            unshifted_codepoint: 97,
            ..KeyEvent::default()
        }),
        "\x1b[97:65;2u".as_bytes()
    );
}

#[test]
fn protocol_matching_unshifted_codepoint() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(5).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyA,
            mods: Mods::SHIFT,
            utf8: "A".into(),
            unshifted_codepoint: 65,
            ..KeyEvent::default()
        }),
        "\x1b[65::97;2u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_with_caps() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyJ,
            mods: Mods::CAPS_LOCK,
            utf8: "J".into(),
            unshifted_codepoint: 106,
            ..KeyEvent::default()
        }),
        "\x1b[106;65;74u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_colon_shift() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Semicolon,
            mods: Mods::SHIFT,
            utf8: ":".into(),
            unshifted_codepoint: ';' as u32,
            ..KeyEvent::default()
        }),
        "\x1b[59:58;2;58u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_with_ru_layout() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Semicolon,
            utf8: "ч".into(),
            unshifted_codepoint: 1095,
            ..KeyEvent::default()
        }),
        "\x1b[1095::59;;1095u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_with_ru_layout_shifted() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Semicolon,
            mods: Mods::SHIFT,
            utf8: "Ч".into(),
            unshifted_codepoint: 1095,
            ..KeyEvent::default()
        }),
        "\x1b[1095:1063:59;2;1063u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_with_ru_layout_caps_lock() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Semicolon,
            mods: Mods::CAPS_LOCK,
            utf8: "Ч".into(),
            unshifted_codepoint: 1095,
            ..KeyEvent::default()
        }),
        "\x1b[1095::59;65;1063u".as_bytes()
    );
}

#[test]
fn protocol_report_alternates_with_hu_layout_release() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::BracketLeft,
            action: KeyAction::Release,
            mods: Mods::CTRL,
            unshifted_codepoint: 337,
            ..KeyEvent::default()
        }),
        "\x1b[337::91;5:3u".as_bytes()
    );
}

#[test]
fn protocol_up_arrow_with_utf8() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::DISAMBIGUATE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ArrowUp,
            utf8: "\x1e".into(),
            ..KeyEvent::default()
        }),
        "\x1b[A".as_bytes()
    );
}

#[test]
fn protocol_shift_tab() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(5).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Tab,
            mods: Mods::SHIFT,
            ..KeyEvent::default()
        }),
        "\x1b[9;2u".as_bytes()
    );
}

#[test]
fn protocol_left_shift() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(5).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ShiftLeft,
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
fn protocol_left_shift_with_report_all() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(9).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::ShiftLeft,
            ..KeyEvent::default()
        }),
        "\x1b[57441u".as_bytes()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn protocol_report_associated_with_alt_text_on_macos_with_option() {
    let mut encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    encoder.macos_option_as_alt = OptionAsAlt::False;
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyW,
            mods: Mods::ALT,
            utf8: "∑".into(),
            unshifted_codepoint: 119,
            ..KeyEvent::default()
        }),
        "\x1b[119;3;8721u".as_bytes()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn protocol_report_associated_with_alt_text_on_macos_with_alt() {
    {
        let mut encoder = KeyEncoder {
            kitty_flags: Flags::from_int(29).unwrap(),
            ..KeyEncoder::default()
        };
        encoder.macos_option_as_alt = OptionAsAlt::True;
        assert_eq!(
            encoder.encode(&KeyEvent {
                key: Key::KeyW,
                mods: Mods::ALT,
                utf8: "∑".into(),
                unshifted_codepoint: 119,
                ..KeyEvent::default()
            }),
            "\x1b[119;3u".as_bytes()
        );
    }
    {
        let mut encoder = KeyEncoder {
            kitty_flags: Flags::from_int(29).unwrap(),
            ..KeyEncoder::default()
        };
        encoder.macos_option_as_alt = OptionAsAlt::True;
        assert_eq!(
            encoder.encode(&KeyEvent {
                key: Key::KeyW,
                utf8: "∑".into(),
                unshifted_codepoint: 119,
                ..KeyEvent::default()
            }),
            "\x1b[119;;8721u".as_bytes()
        );
    }
}

#[test]
fn protocol_report_associated_with_modifiers() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyJ,
            mods: Mods::CTRL,
            utf8: "j".into(),
            unshifted_codepoint: 106,
            ..KeyEvent::default()
        }),
        "\x1b[106;5u".as_bytes()
    );
}

#[test]
fn protocol_report_associated() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(29).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyJ,
            mods: Mods::SHIFT,
            utf8: "J".into(),
            unshifted_codepoint: 106,
            ..KeyEvent::default()
        }),
        "\x1b[106:74;2;74u".as_bytes()
    );
}

#[test]
fn protocol_report_associated_on_release() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::KeyJ,
            action: KeyAction::Release,
            mods: Mods::SHIFT,
            utf8: "J".into(),
            unshifted_codepoint: 106,
            ..KeyEvent::default()
        }),
        "\x1b[106:74;2:3u".as_bytes()
    );
}

#[test]
fn protocol_alternates_omit_control_characters() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(13).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Delete,
            utf8: "\x7f".into(),
            ..KeyEvent::default()
        }),
        "\x1b[3~".as_bytes()
    );
}

#[test]
fn protocol_enter_with_utf8_dead_key_state() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::from_int(13).unwrap(),
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Enter,
            utf8: "A".into(),
            unshifted_codepoint: 0x0D,
            ..KeyEvent::default()
        }),
        "A".as_bytes()
    );
}

#[test]
fn protocol_keypad_number() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Numpad1,
            utf8: "1".into(),
            ..KeyEvent::default()
        }),
        "\x1b[57400;;49u".as_bytes()
    );
}

#[test]
fn protocol_backspace_with_utf8_dead_key_state() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Backspace,
            utf8: "A".into(),
            unshifted_codepoint: 0x0D,
            ..KeyEvent::default()
        }),
        "".as_bytes()
    );
}

#[test]
fn protocol_backspace_decbkm_reset_report_all_true() {
    let encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Backspace,
            ..KeyEvent::default()
        }),
        "\x1b[127u".as_bytes()
    );
}

#[test]
fn protocol_backspace_decbkm_set_report_all_true() {
    let mut encoder = KeyEncoder {
        kitty_flags: Flags::TRUE,
        ..KeyEncoder::default()
    };
    encoder.modes.set(Mode::BackarrowKeyMode, true);
    assert_eq!(
        encoder.encode(&KeyEvent {
            key: Key::Backspace,
            ..KeyEvent::default()
        }),
        "\x1b[127u".as_bytes()
    );
}

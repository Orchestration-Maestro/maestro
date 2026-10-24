//! Semantic key policy through the engine's shared formatter.
use crate::input::{KeyboardProtocol, TerminalKey, parse_terminal_key_sequence};
use crate::panes::input::encode_terminal_key;
use maestro_protocol::frames::{ClientKeyCode, ClientKeyKind, KeyModifiers};

/// Check parser observations after an encoding roundtrip.
fn assert_terminal_key_eq(
    actual: &TerminalKey,
    code: &ClientKeyCode,
    modifiers: u8,
    kind: ClientKeyKind,
    shifted_codepoint: Option<u32>,
) {
    assert_eq!(&actual.code, code);
    assert_eq!(actual.modifiers, modifiers);
    assert_eq!(actual.kind, kind);
    assert_eq!(actual.shifted_codepoint, shifted_codepoint);
}

#[test]
fn pane_policy_legacy_enter() {
    let key = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        vec![b'\r']
    );
}

#[test]
fn pane_policy_legacy_ctrl_c() {
    let key = TerminalKey::new(ClientKeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(encode_terminal_key(&key, KeyboardProtocol::Legacy), vec![3]);
}

#[test]
fn pane_policy_legacy_ctrl_slash_aliases_ctrl_underscore() {
    let key = TerminalKey::new(ClientKeyCode::Char('/'), KeyModifiers::CONTROL);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        vec![31]
    );
}

#[test]
fn pane_policy_legacy_shift_enter_is_just_cr() {
    let key = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        vec![b'\r']
    );
}

#[test]
fn pane_policy_legacy_alt_up() {
    let key = TerminalKey::new(ClientKeyCode::Up, KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[1;3A"
    );
}

#[test]
fn pane_policy_legacy_shift_right() {
    let key = TerminalKey::new(ClientKeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[1;2C"
    );
}

#[test]
fn pane_policy_legacy_ctrl_left() {
    let key = TerminalKey::new(ClientKeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[1;5D"
    );
}

#[test]
fn pane_policy_legacy_ctrl_shift_end() {
    let key = TerminalKey::new(
        ClientKeyCode::End,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[1;6F"
    );
}

#[test]
fn pane_policy_legacy_alt_delete() {
    let key = TerminalKey::new(ClientKeyCode::Delete, KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[3;3~"
    );
}

#[test]
fn pane_policy_legacy_shift_f5() {
    let key = TerminalKey::new(ClientKeyCode::F(5), KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b[15;2~"
    );
}

#[test]
fn pane_policy_legacy_alt_char_still_esc_prefix() {
    let key = TerminalKey::new(ClientKeyCode::Char('a'), KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1ba"
    );
}

#[test]
fn pane_policy_legacy_alt_shift_punctuation_uses_shifted_text() {
    let key = parse_terminal_key_sequence("\x1b[44:60;4u").unwrap();
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b<"
    );
}

#[test]
fn pane_policy_legacy_alt_backspace_sends_escape_delete() {
    let key = TerminalKey::new(ClientKeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Legacy),
        b"\x1b\x7f"
    );
}

#[test]
fn pane_policy_kitty_shift_enter() {
    let key = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[13;2u"
    );
}

#[test]
fn pane_policy_kitty_ctrl_shift_a() {
    let key = TerminalKey::new(
        ClientKeyCode::Char('a'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[97;6u"
    );
}

#[test]
fn pane_policy_kitty_shift_uppercase_letter_sends_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('L'), KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"L"
    );
}

#[test]
fn pane_policy_kitty_shift_uppercase_letter_ignores_alternate_key_reporting_for_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('L'), KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 }),
        b"L"
    );
}

#[test]
fn pane_policy_kitty_shift_lowercase_letter_sends_uppercase_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('l'), KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"L"
    );
}

#[test]
fn pane_policy_kitty_alt_shift_uppercase_letter_uses_base_codepoint() {
    let key = TerminalKey::new(
        ClientKeyCode::Char('L'),
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    );
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[108;4u"
    );
}

#[test]
fn pane_policy_kitty_ctrl_shift_uppercase_letter_uses_base_codepoint() {
    let key = TerminalKey::new(
        ClientKeyCode::Char('L'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[108;6u"
    );
}

#[test]
fn pane_policy_legacy_shift_uppercase_letter_stays_uppercase() {
    let key = TerminalKey::new(ClientKeyCode::Char('L'), KeyModifiers::SHIFT);
    assert_eq!(encode_terminal_key(&key, KeyboardProtocol::Legacy), b"L");
}

#[test]
fn pane_policy_kitty_alt_enter() {
    let key = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[13;3u"
    );
}

#[test]
fn pane_policy_kitty_alt_backspace_uses_csi_u() {
    let key = TerminalKey::new(ClientKeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[127;3u"
    );
}

#[test]
fn pane_policy_kitty_plain_ctrl_c_uses_csi_u() {
    let key = TerminalKey::new(ClientKeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[99;5u"
    );
}

#[test]
fn pane_policy_kitty_plain_ctrl_c_includes_press_event_when_requested() {
    let key = TerminalKey::new(ClientKeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[99;5:1u"
    );
}

#[test]
fn pane_policy_kitty_unmodified_uses_legacy() {
    let key = TerminalKey::new(ClientKeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"a"
    );
}

#[test]
fn pane_policy_kitty_report_event_types_keeps_basic_compatibility_keys_legacy() {
    let cases = [
        (ClientKeyCode::Enter, b"\r".as_slice()),
        (ClientKeyCode::Tab, b"\t".as_slice()),
        (ClientKeyCode::Backspace, b"\x7f".as_slice()),
    ];

    for (code, expected) in cases {
        let press =
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Press);
        assert_eq!(
            encode_terminal_key(&press, KeyboardProtocol::Kitty { flags: 3 }),
            expected,
            "{code:?} press should stay legacy-compatible without REPORT_ALL_KEYS"
        );

        let repeat =
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Repeat);
        assert_eq!(
            encode_terminal_key(&repeat, KeyboardProtocol::Kitty { flags: 3 }),
            expected,
            "{code:?} repeat should stay legacy-compatible without REPORT_ALL_KEYS"
        );

        let release =
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Release);
        assert_eq!(
            encode_terminal_key(&release, KeyboardProtocol::Kitty { flags: 3 }),
            b"",
            "{code:?} release should not fall back to legacy bytes"
        );
    }
}

#[test]
fn pane_policy_kitty_report_all_keys_encodes_basic_compatibility_keys_with_events() {
    let enter_press =
        TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::NONE).with_kind(ClientKeyKind::Press);
    assert_eq!(
        encode_terminal_key(&enter_press, KeyboardProtocol::Kitty { flags: 9 }),
        b"\x1b[13;1u"
    );

    let backspace_press = TerminalKey::new(ClientKeyCode::Backspace, KeyModifiers::NONE)
        .with_kind(ClientKeyKind::Press);
    assert_eq!(
        encode_terminal_key(&backspace_press, KeyboardProtocol::Kitty { flags: 11 }),
        b"\x1b[127;1:1u"
    );

    let backspace_release = TerminalKey::new(ClientKeyCode::Backspace, KeyModifiers::NONE)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(
        encode_terminal_key(&backspace_release, KeyboardProtocol::Kitty { flags: 11 }),
        b"\x1b[127;1:3u"
    );
}

#[test]
fn pane_policy_kitty_report_all_keys_encodes_printable_event_kinds() {
    for (kind, expected) in [
        (ClientKeyKind::Press, b"\x1b[106;1:1u".as_slice()),
        (ClientKeyKind::Repeat, b"\x1b[106;1:2u".as_slice()),
        (ClientKeyKind::Release, b"\x1b[106;1:3u".as_slice()),
    ] {
        let key = TerminalKey::new(ClientKeyCode::Char('j'), KeyModifiers::NONE).with_kind(kind);
        assert_eq!(
            encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 15 }),
            expected
        );
    }
}

#[test]
fn pane_policy_kitty_report_associated_text_embeds_shifted_printables() {
    let cases = [
        (
            TerminalKey::new(ClientKeyCode::Char('A'), KeyModifiers::SHIFT),
            b"\x1b[97;2;65u".as_slice(),
        ),
        (
            TerminalKey::new(ClientKeyCode::Char('1'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('!' as u32),
            b"\x1b[49;2;33u".as_slice(),
        ),
        (
            TerminalKey::new(ClientKeyCode::Char(':'), KeyModifiers::SHIFT),
            b"\x1b[58;2;58u".as_slice(),
        ),
    ];

    for (key, expected) in cases {
        assert_eq!(
            encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 25 }),
            expected
        );
    }
}

#[test]
fn pane_policy_kitty_associated_text_composes_with_alternates_and_events() {
    for (kind, expected) in [
        (ClientKeyKind::Press, b"\x1b[97:65;2:1;65u".as_slice()),
        (ClientKeyKind::Repeat, b"\x1b[97:65;2:2;65u".as_slice()),
        (ClientKeyKind::Release, b"\x1b[97:65;2:3u".as_slice()),
    ] {
        let key = TerminalKey::new(ClientKeyCode::Char('A'), KeyModifiers::SHIFT).with_kind(kind);
        assert_eq!(
            encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 31 }),
            expected
        );
    }
}

#[test]
fn pane_policy_kitty_printable_release_is_encoded_without_report_all() {
    let release = TerminalKey::new(ClientKeyCode::Char('j'), KeyModifiers::NONE)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(
        encode_terminal_key(&release, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[106;1:3u"
    );

    let mut malformed_release = release;
    malformed_release.generated_text = Some("j".to_owned());
    assert_eq!(
        encode_terminal_key(&malformed_release, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[106;1:3u"
    );
}

#[test]
fn pane_policy_kitty_shift_tab() {
    let key = TerminalKey::new(ClientKeyCode::Tab, KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[9;2u"
    );
}

#[test]
fn pane_policy_kitty_ctrl_shift_enter() {
    let key = TerminalKey::new(
        ClientKeyCode::Enter,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 1 }),
        b"\x1b[13;6u"
    );
}

#[test]
fn pane_policy_kitty_repeat_event_type_is_encoded_when_requested() {
    let key = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::SHIFT)
        .with_kind(ClientKeyKind::Repeat);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[13;2:2u"
    );
}

#[test]
fn pane_policy_kitty_shift_letter_release_uses_csi_u() {
    let key = TerminalKey::new(ClientKeyCode::Char('L'), KeyModifiers::SHIFT)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 }),
        b"\x1b[108:76;2:3u"
    );
}

#[test]
fn pane_policy_kitty_shifted_punctuation_literals_send_text() {
    for ch in "!@#$%^&*()_+{}|:\"<>?~".chars() {
        let key = TerminalKey::new(ClientKeyCode::Char(ch), KeyModifiers::SHIFT);
        let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
        assert_eq!(encoded, ch.to_string().into_bytes(), "ch={ch}");
    }
}

#[test]
fn pane_policy_kitty_shifted_punctuation_release_does_not_emit_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('?'), KeyModifiers::SHIFT)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 }),
        b"\x1b[63;2:3u"
    );
}

#[test]
fn pane_policy_kitty_shifted_punctuation_does_not_infer_layout() {
    let key = TerminalKey::new(ClientKeyCode::Char('1'), KeyModifiers::SHIFT);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 }),
        b"\x1b[49;2:1u"
    );
}

#[test]
fn pane_policy_kitty_modified_shifted_punctuation_stays_modified_key() {
    for (modifiers, expected) in [
        (
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            b"\x1b[33;6:1u".as_slice(),
        ),
        (
            KeyModifiers::ALT | KeyModifiers::SHIFT,
            b"\x1b[33;4:1u".as_slice(),
        ),
        (
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
            b"\x1b[33;10:1u".as_slice(),
        ),
    ] {
        let key = TerminalKey::new(ClientKeyCode::Char('!'), modifiers);
        let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
        assert_eq!(encoded, expected, "modifiers={modifiers:?}");
    }
}

#[test]
fn pane_policy_release_bytes_gated_on_report_event_types() {
    for code in [ClientKeyCode::Enter, ClientKeyCode::Backspace] {
        let release =
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Release);

        assert_eq!(encode_terminal_key(&release, KeyboardProtocol::Legacy), b"");
        assert_eq!(
            encode_terminal_key(&release, KeyboardProtocol::Kitty { flags: 1 }),
            b""
        );
    }

    let modified_release = TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::CONTROL)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(
        encode_terminal_key(&modified_release, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[13;5:3u"
    );
}

#[test]
fn pane_policy_kitty_shifted_symbol_sends_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('1'), KeyModifiers::SHIFT)
        .with_shifted_codepoint('!' as u32);
    assert_eq!(
        encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 }),
        b"!"
    );
}

#[test]
fn pane_policy_legacy_modified_special_roundtrip_matrix() {
    let cases = [
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::ALT),
        TerminalKey::new(ClientKeyCode::Down, KeyModifiers::ALT),
        TerminalKey::new(ClientKeyCode::Right, KeyModifiers::SHIFT),
        TerminalKey::new(ClientKeyCode::Left, KeyModifiers::CONTROL),
        TerminalKey::new(ClientKeyCode::Home, KeyModifiers::CONTROL),
        TerminalKey::new(
            ClientKeyCode::End,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        TerminalKey::new(ClientKeyCode::PageUp, KeyModifiers::ALT),
        TerminalKey::new(ClientKeyCode::PageDown, KeyModifiers::CONTROL),
        TerminalKey::new(ClientKeyCode::Insert, KeyModifiers::SHIFT),
        TerminalKey::new(ClientKeyCode::Delete, KeyModifiers::ALT),
    ];

    for key in cases {
        let encoded = encode_terminal_key(&key, KeyboardProtocol::Legacy);
        let parsed = parse_terminal_key_sequence(std::str::from_utf8(&encoded).unwrap()).unwrap();
        assert_terminal_key_eq(&parsed, &key.code, key.modifiers, key.kind, None);
    }
}

#[test]
fn pane_policy_kitty_shifted_symbol_prefers_text_over_roundtrip_key_identity() {
    let key = TerminalKey::new(ClientKeyCode::Char('1'), KeyModifiers::SHIFT)
        .with_shifted_codepoint('!' as u32);
    let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
    assert_eq!(encoded, b"!");
}

#[test]
fn pane_policy_legacy_basic_special_roundtrip_matrix() {
    let cases = [
        TerminalKey::new(ClientKeyCode::Enter, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Tab, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Backspace, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Esc, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Down, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Left, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Right, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Home, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::End, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::PageUp, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::PageDown, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Insert, KeyModifiers::NONE),
        TerminalKey::new(ClientKeyCode::Delete, KeyModifiers::NONE),
    ];

    for key in cases {
        let encoded = encode_terminal_key(&key, KeyboardProtocol::Legacy);
        let parsed = parse_terminal_key_sequence(std::str::from_utf8(&encoded).unwrap()).unwrap();
        assert_terminal_key_eq(&parsed, &key.code, key.modifiers, key.kind, None);
    }
}

#[test]
fn pane_policy_kitty_shifted_symbol_pair_matrix_is_encoded_as_text() {
    let cases = [('1', '!'), ('/', '?'), ('[', '{')];

    for (base, shifted) in cases {
        let key = TerminalKey::new(ClientKeyCode::Char(base), KeyModifiers::SHIFT)
            .with_shifted_codepoint(shifted as u32);
        let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
        assert_eq!(encoded, shifted.to_string().into_bytes(), "base={base}");
    }
}

#[test]
fn pane_policy_chinese_char_encodes_as_utf8() {
    let key = TerminalKey::new(ClientKeyCode::Char('中'), KeyModifiers::NONE);
    let encoded = encode_terminal_key(&key, KeyboardProtocol::Legacy);
    assert_eq!(encoded, "中".as_bytes());
}

#[test]
fn pane_policy_chinese_char_with_kitty_protocol_encodes_as_utf8() {
    let key = TerminalKey::new(ClientKeyCode::Char('文'), KeyModifiers::NONE);
    let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
    assert_eq!(encoded, "文".as_bytes());
}

#[test]
fn pane_policy_chinese_char_with_modifiers_falls_back_to_kitty_encoding() {
    let key = TerminalKey::new(ClientKeyCode::Char('测'), KeyModifiers::ALT);
    let encoded = encode_terminal_key(&key, KeyboardProtocol::Kitty { flags: 7 });
    assert!(!encoded.is_empty());
    assert_ne!(encoded, "测".as_bytes());
}

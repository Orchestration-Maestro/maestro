//! Key input boundary observations.

use super::*;
use crate::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
use maestro_protocol::frames::{
    ClientInputEvent, ClientKeyCode, ClientKeyKind, ClientKeySource, KeyModifiers,
};

/// Enhanced scan identity remains separate from the semantic code.
#[test]
fn native_keys_keep_scan_identity_and_semantic_code() {
    let record = WindowsKeyRecord {
        key_down: true,
        repeat_count: 1,
        virtual_key_code: 27,
        virtual_scan_code: 1,
        unicode: 27,
        control_key_state: 0,
    };
    let key = TerminalKey::new(ClientKeyCode::Esc, KeyModifiers::NONE).with_windows_record(record);
    let enhanced = TerminalKey::new(ClientKeyCode::Esc, KeyModifiers::NONE).with_windows_record(
        WindowsKeyRecord {
            control_key_state: 0x0100,
            ..record
        },
    );
    assert!(matches!(key.identity(), KeyIdentity::Physical(_)));
    assert_ne!(key.identity(), enhanced.identity());
    assert_eq!(key.code, ClientKeyCode::Esc);
    assert_eq!(enhanced.code, ClientKeyCode::Esc);
}

/// Synthetic, VT and zero-scan sources fall back to semantic identity.
#[test]
fn synthetic_keys_use_semantic_identity() {
    let code = ClientKeyCode::Char('x');
    let key = TerminalKey::new(code.clone(), KeyModifiers::CONTROL);
    assert_eq!(key.windows_record(), None);
    let vt = key.clone().with_vt_bytes(b"x".to_vec());
    let native = key.clone().with_windows_record(WindowsKeyRecord {
        key_down: true,
        repeat_count: 1,
        virtual_key_code: 88,
        virtual_scan_code: 0,
        unicode: 120,
        control_key_state: 0x0100,
    });
    for key in [key, vt, native] {
        assert_eq!(key.identity(), KeyIdentity::Semantic(code.clone()));
        assert!(!key.has_physical_identity());
    }
}

/// Canonical release never rewrites native key-down evidence.
#[test]
fn canonical_release_keeps_native_record() {
    let record = WindowsKeyRecord {
        key_down: true,
        repeat_count: 5,
        virtual_key_code: 27,
        virtual_scan_code: 1,
        unicode: 27,
        control_key_state: 0,
    };
    let key = TerminalKey::new(ClientKeyCode::Esc, KeyModifiers::NONE)
        .with_windows_record(record)
        .with_kind(maestro_protocol::frames::ClientKeyKind::Release);
    assert_eq!(key.kind, maestro_protocol::frames::ClientKeyKind::Release);
    assert_eq!(key.windows_record(), Some(record));
    assert_eq!(key.repeat_count, 1);
}

/// Release normalization applies in either builder order.
#[test]
fn release_discards_text_and_resets_repetition() {
    use maestro_protocol::frames::ClientKeyKind;
    let release = TerminalKey::new(ClientKeyCode::Char('a'), 0)
        .with_generated_text(Some("a".into()))
        .with_repeat_count(4)
        .with_kind(ClientKeyKind::Release);
    assert_eq!(release.generated_text, None);
    assert_eq!(release.repeat_count, 1);
    let regrouped = release
        .with_repeat_count(4)
        .with_generated_text(Some("ignored".into()));
    assert_eq!(regrouped.generated_text, None);
    assert_eq!(regrouped.repeat_count, 1);
}

/// Shift does not suppress an uppercase Unicode commit.
#[test]
fn shifted_unicode_uppercase_is_text() {
    let key = TerminalKey::new(ClientKeyCode::Char('É'), KeyModifiers::SHIFT).with_text_commit();
    assert_eq!(key.generated_text.as_deref(), Some("É"));
}

/// Zero extended flags retain legacy keyboard handling.
#[test]
fn zero_keyboard_flags_select_legacy() {
    assert_eq!(
        KeyboardProtocol::from_kitty_flags(0),
        KeyboardProtocol::Legacy
    );
}

/// Nonzero extended flags are retained without filtering.
#[test]
fn nonzero_keyboard_flags_keep_extended_bits() {
    assert_eq!(
        KeyboardProtocol::from_kitty_flags(7),
        KeyboardProtocol::Kitty { flags: 7 }
    );
}

/// Builders retain press/repeat fields while replacing transport sources.
#[test]
fn key_builders_preserve_press_repeat_and_source() {
    use maestro_protocol::frames::ClientKeyKind;
    let key = TerminalKey::new(ClientKeyCode::Char('a'), 0);
    assert_key_defaults(&key);
    for kind in [ClientKeyKind::Press, ClientKeyKind::Repeat] {
        let changed = key
            .clone()
            .with_kind(kind)
            .with_repeat_count(4)
            .with_generated_text(Some("a".into()))
            .with_modifiers(37)
            .with_shifted_codepoint(0x00c9);
        assert_eq!(changed.kind, kind);
        assert_eq!(changed.repeat_count, 4);
        assert_eq!(changed.generated_text.as_deref(), Some("a"));
        assert_eq!(changed.modifiers, 37);
        assert_eq!(changed.shifted_codepoint, Some(0x00c9));
    }
    let record = WindowsKeyRecord {
        key_down: true,
        repeat_count: 3,
        virtual_key_code: 65,
        virtual_scan_code: 30,
        unicode: 97,
        control_key_state: 0,
    };
    let native = key.with_vt_bytes(vec![97]).with_windows_record(record);
    assert_eq!(native.windows_record(), Some(record));
    assert_eq!(native.vt_bytes(), None);
    assert_eq!(native.repeat_count, 3);
    let vt = native.with_vt_bytes(vec![27, 97]);
    assert_eq!(vt.windows_record(), None);
    assert_eq!(vt.vt_bytes(), Some([27, 97].as_slice()));
}

/// Every protocol code, phase and source survives the owning conversion pair.
#[test]
fn key_wire_conversion_covers_sources_kinds_and_codes() {
    let codes = [
        ClientKeyCode::Backspace,
        ClientKeyCode::Enter,
        ClientKeyCode::Left,
        ClientKeyCode::Right,
        ClientKeyCode::Up,
        ClientKeyCode::Down,
        ClientKeyCode::Home,
        ClientKeyCode::End,
        ClientKeyCode::PageUp,
        ClientKeyCode::PageDown,
        ClientKeyCode::Tab,
        ClientKeyCode::BackTab,
        ClientKeyCode::Delete,
        ClientKeyCode::Insert,
        ClientKeyCode::Esc,
        ClientKeyCode::Char('é'),
        ClientKeyCode::F(12),
        ClientKeyCode::Null,
    ];
    let record = WindowsKeyRecord {
        key_down: true,
        repeat_count: 7,
        virtual_key_code: 65,
        virtual_scan_code: 30,
        unicode: 97,
        control_key_state: 0x0100,
    };
    let sources = [
        ClientKeySource::Synthesized,
        ClientKeySource::Vt {
            bytes: vec![27, 97],
        },
        ClientKeySource::WindowsConsole { record },
    ];
    for code in codes {
        for kind in [
            ClientKeyKind::Press,
            ClientKeyKind::Repeat,
            ClientKeyKind::Release,
        ] {
            for source in &sources {
                assert_key_wire_roundtrip(&code, kind, source);
            }
        }
    }
}

/// Narrowing returns every non-key payload to its caller unchanged.
#[test]
fn non_key_wire_events_return_unchanged() {
    use maestro_protocol::frames::{ClientInputEvent, ClientMouseButton, ClientMouseKind};
    for event in [
        ClientInputEvent::TextCommit("é".into()),
        ClientInputEvent::Mouse {
            kind: ClientMouseKind::Drag(ClientMouseButton::Right),
            column: 17,
            row: 8,
            modifiers: 5,
        },
        ClientInputEvent::Paste {
            text: "paste\n".into(),
        },
        ClientInputEvent::FocusGained,
        ClientInputEvent::FocusLost,
    ] {
        assert_eq!(TerminalKey::try_from(event.clone()), Err(event));
    }
}

/// Wire repetition and release policy override native repetition only canonically.
#[test]
fn key_wire_ingress_normalizes_flags_and_counts() {
    let record = WindowsKeyRecord {
        key_down: true,
        repeat_count: 7,
        virtual_key_code: 65,
        virtual_scan_code: 30,
        unicode: 97,
        control_key_state: 0x0100,
    };
    for (kind, count, expected_count, expected_text) in [
        (ClientKeyKind::Press, 4, 4, Some("a")),
        (ClientKeyKind::Repeat, 0, 1, Some("a")),
        (ClientKeyKind::Release, 4, 1, None),
    ] {
        let key = TerminalKey::try_from(ClientInputEvent::Key {
            code: ClientKeyCode::Char('a'),
            modifiers: 0xff,
            kind,
            repeat_count: count,
            generated_text: Some("a".into()),
            source: ClientKeySource::WindowsConsole { record },
        })
        .unwrap();
        assert_eq!(key.modifiers, 0x3f);
        assert_eq!(key.repeat_count, expected_count);
        assert_eq!(key.generated_text.as_deref(), expected_text);
        assert_eq!(key.windows_record(), Some(record));
    }
}

/// Semantic snapshots deliberately exclude transport evidence and generated text.
#[test]
fn semantic_key_view_has_only_semantics() {
    let key = TerminalKey::new(ClientKeyCode::Char('é'), 5)
        .with_kind(ClientKeyKind::Repeat)
        .with_repeat_count(3)
        .with_generated_text(Some("é".into()))
        .with_vt_bytes(vec![27, 97])
        .with_shifted_codepoint(0x00c9);
    assert_eq!(
        key.as_key_event(),
        ClientInputEvent::Key {
            code: ClientKeyCode::Char('é'),
            modifiers: 5,
            kind: ClientKeyKind::Repeat,
            repeat_count: 1,
            generated_text: None,
            source: ClientKeySource::Synthesized
        }
    );
    assert_eq!(
        ClientInputEvent::from(key),
        ClientInputEvent::Key {
            code: ClientKeyCode::Char('é'),
            modifiers: 5,
            kind: ClientKeyKind::Repeat,
            repeat_count: 3,
            generated_text: Some("é".into()),
            source: ClientKeySource::Vt {
                bytes: vec![27, 97]
            }
        }
    );
}

/// Event-type and all-key reporting read distinct flag bits.
#[test]
fn keyboard_flags_are_independent() {
    for (flags, events, all) in [
        (0, false, false),
        (1, false, false),
        (2, true, false),
        (8, false, true),
        (10, true, true),
        (65535, true, true),
    ] {
        let protocol = KeyboardProtocol::from_kitty_flags(flags);
        assert_eq!(protocol.reports_event_types(), events);
        assert_eq!(protocol.reports_all_keys(), all);
    }
}

/// Host keyboard mode commands contain exactly their escape bytes.
#[test]
fn modify_other_keys_sequences_match_modes() {
    assert_eq!(ModifyOtherKeysMode::Mode1.set_sequence(), b"\x1b[>4;1m");
    assert_eq!(ModifyOtherKeysMode::Mode2.set_sequence(), b"\x1b[>4;2m");
}

/// Mouse modes and encodings retain their serialized names.
#[test]
fn mouse_protocol_variants_keep_snake_case() {
    for (mode, name) in [
        (MouseProtocolMode::None, "none"),
        (MouseProtocolMode::Press, "press"),
        (MouseProtocolMode::PressRelease, "press_release"),
        (MouseProtocolMode::ButtonMotion, "button_motion"),
        (MouseProtocolMode::AnyMotion, "any_motion"),
    ] {
        let json = format!("\"{name}\"");
        assert_eq!(serde_json::to_string(&mode).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<MouseProtocolMode>(&json).unwrap(),
            mode
        );
    }
    for (encoding, name) in [
        (MouseProtocolEncoding::Default, "default"),
        (MouseProtocolEncoding::Utf8, "utf8"),
        (MouseProtocolEncoding::Sgr, "sgr"),
        (MouseProtocolEncoding::SgrPixels, "sgr_pixels"),
    ] {
        let json = format!("\"{name}\"");
        assert_eq!(serde_json::to_string(&encoding).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<MouseProtocolEncoding>(&json).unwrap(),
            encoding
        );
    }
}

/// Committed text owns its empty or Unicode contents.
#[test]
fn text_commit_keeps_owned_unicode() {
    for value in ["", "é\n音"] {
        let original = value.to_owned();
        let commit = TextCommit::new(original.as_str());
        drop(original);
        assert_eq!(commit.as_str(), value);
        assert_eq!(commit.into_string(), value);
    }
}

/// Inference respects phase and modifiers, leaving supplied text on other paths.
#[test]
fn text_commit_inference_respects_phase_and_modifiers() {
    use maestro_protocol::frames::ClientKeyKind;
    for (code, modifiers, expected) in [
        (ClientKeyCode::Char('a'), 0, Some("a")),
        (ClientKeyCode::Char('A'), 0, Some("A")),
        (ClientKeyCode::Char('音'), 0, Some("音")),
        (ClientKeyCode::Char('A'), 1, Some("A")),
        (ClientKeyCode::Char('a'), 1, None),
        (ClientKeyCode::Char('A'), 2, None),
        (ClientKeyCode::Char('A'), 4, None),
        (ClientKeyCode::Char('A'), 3, None),
        (ClientKeyCode::Enter, 0, None),
    ] {
        for kind in [
            ClientKeyKind::Press,
            ClientKeyKind::Repeat,
            ClientKeyKind::Release,
        ] {
            let base = TerminalKey::new(code.clone(), modifiers).with_kind(kind);
            let inferred = base.clone().with_text_commit();
            let expected = if kind == ClientKeyKind::Press {
                expected
            } else {
                None
            };
            assert_eq!(inferred.generated_text.as_deref(), expected);
            let supplied = base
                .with_generated_text(Some("supplied".into()))
                .with_text_commit();
            let expected = if kind == ClientKeyKind::Release {
                None
            } else {
                expected.or(Some("supplied"))
            };
            assert_eq!(supplied.generated_text.as_deref(), expected);
        }
    }
}

/// Decoded variants retain their owned payloads without alternate record types.
#[test]
fn raw_input_events_keep_all_payloads() {
    assert_raw_key_text_and_mouse();
    assert!(matches!(
        RawInputEvent::OuterFocusGained,
        RawInputEvent::OuterFocusGained
    ));
    assert!(matches!(
        RawInputEvent::OuterFocusLost,
        RawInputEvent::OuterFocusLost
    ));
    assert_raw_host_colors();
    for appearance in [HostAppearance::Dark, HostAppearance::Light] {
        let event = RawInputEvent::HostColorSchemeChanged(appearance);
        assert!(
            matches!(event, RawInputEvent::HostColorSchemeChanged(actual) if actual == appearance)
        );
    }
    let size = RawInputEvent::HostCellSizeReport {
        width_px: 9,
        height_px: 17,
    };
    assert!(matches!(
        size,
        RawInputEvent::HostCellSizeReport {
            width_px: 9,
            height_px: 17
        }
    ));
    assert!(matches!(
        RawInputEvent::Unsupported,
        RawInputEvent::Unsupported
    ));
}

/// Observe one owning conversion combination through both directions.
fn assert_key_wire_roundtrip(code: &ClientKeyCode, kind: ClientKeyKind, source: &ClientKeySource) {
    let input = ClientInputEvent::Key {
        code: code.clone(),
        modifiers: 37,
        kind,
        repeat_count: 4,
        generated_text: Some("é".into()),
        source: source.clone(),
    };
    let key = TerminalKey::try_from(input).unwrap();
    assert_eq!(&key.code, code);
    assert_eq!(key.modifiers, 37);
    assert_eq!(key.kind, kind);
    assert_eq!(key.shifted_codepoint, None);
    let (count, text) = if kind == ClientKeyKind::Release {
        (1, None)
    } else {
        (4, Some("é".to_owned()))
    };
    assert_eq!(key.repeat_count, count);
    assert_eq!(key.generated_text, text);
    let expected = ClientInputEvent::Key {
        code: code.clone(),
        modifiers: 37,
        kind,
        repeat_count: count,
        generated_text: text,
        source: source.clone(),
    };
    assert_eq!(ClientInputEvent::from(key), expected);
}

/// Observe constructor defaults before any builder changes them.
fn assert_key_defaults(key: &TerminalKey) {
    assert_eq!(key.kind, ClientKeyKind::Press);
    assert_eq!(key.repeat_count, 1);
    assert_eq!(key.generated_text, None);
    assert_eq!(key.shifted_codepoint, None);
    assert_eq!(key.vt_bytes(), None);
    assert_eq!(key.windows_record(), None);
    assert_eq!(key.clone().with_repeat_count(0).repeat_count, 1);
}

/// Observe ordered host palette entries and the default colour selector.
fn assert_raw_host_colors() {
    let color = RgbColor {
        r: 12,
        g: 34,
        b: 56,
    };
    let default = RawInputEvent::HostDefaultColor {
        kind: DefaultColorKind::Background,
        color,
    };
    assert!(matches!(
        default,
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Background,
            color: RgbColor {
                r: 12,
                g: 34,
                b: 56
            }
        }
    ));
    let palette = RawInputEvent::HostPaletteColors {
        colors: vec![
            (7, color),
            (
                2,
                RgbColor {
                    r: 98,
                    g: 76,
                    b: 54,
                },
            ),
        ],
    };
    assert!(
        matches!(palette, RawInputEvent::HostPaletteColors { colors }
        if colors == [(7, color), (2, RgbColor { r: 98, g: 76, b: 54 })])
    );
}

/// Observe decoded key, text, paste, mouse and focus variants.
fn assert_raw_key_text_and_mouse() {
    use maestro_protocol::frames::{ClientMouseButton, ClientMouseKind};
    let key = RawInputEvent::Key(TerminalKey::new(ClientKeyCode::Char('q'), 4));
    assert!(
        matches!(key, RawInputEvent::Key(key) if key.code == ClientKeyCode::Char('q') && key.modifiers == 4)
    );
    let text = RawInputEvent::Text(TextCommit::new("é"));
    assert!(matches!(text, RawInputEvent::Text(text) if text.as_str() == "é"));
    let paste = RawInputEvent::Paste("a\nb".into());
    assert!(matches!(paste, RawInputEvent::Paste(text) if text == "a\nb"));
    let mouse = RawInputEvent::Mouse {
        kind: ClientMouseKind::Down(ClientMouseButton::Left),
        column: 9,
        row: 3,
        modifiers: 5,
    };
    assert!(matches!(
        mouse,
        RawInputEvent::Mouse {
            kind: ClientMouseKind::Down(ClientMouseButton::Left),
            column: 9,
            row: 3,
            modifiers: 5
        }
    ));
}

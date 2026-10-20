//! Live pane keyboard and mouse encoding.
use super::super::{InputMode, InputState, PaneTerminal};
use crate::input::{KeyboardProtocol, TerminalKey};
use maestro_protocol::frames::{
    ClientKeyCode, ClientKeyKind, ClientMouseButton, ClientMouseKind, KeyModifiers,
};
use maestro_terminal::Terminal;

#[test]
fn pane_keyboard_protocol_tracks_live_terminal_flags() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>3u");
    let pane = PaneTerminal::new(terminal);

    assert_eq!(
        pane.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 3 }
    );
}

#[test]
fn pane_plain_text_chars_still_encode_as_text() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Char('a'), KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );

    assert_eq!(encoded, b"a");
}

#[test]
fn pane_backtab_preserves_shift_across_keyboard_protocols() {
    for (kitty_flags, expected) in [
        (None, b"\x1b[Z".as_slice()),
        (Some(1), b"\x1b[9;2u".as_slice()),
    ] {
        let mut terminal = Terminal::new(80, 24, 0).unwrap();
        if let Some(flags) = kitty_flags {
            terminal.write(format!("\x1b[>{flags}u").as_bytes());
        }
        let pane = PaneTerminal::new(terminal);
        let protocol = pane.keyboard_protocol(KeyboardProtocol::Legacy);

        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            let encoded = pane.encode_terminal_key(
                TerminalKey::new(ClientKeyCode::BackTab, modifiers),
                protocol,
            );
            assert_eq!(encoded, expected, "backtab with modifiers {modifiers:?}");
        }
    }

    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let encoded = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Tab, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );
    assert_eq!(encoded, b"\t");
}

#[test]
fn pane_ctrl_tab_matches_the_pane_keyboard_protocol() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let legacy = PaneTerminal::new(terminal);
    let key = TerminalKey::new(ClientKeyCode::Tab, KeyModifiers::CONTROL);

    assert_eq!(
        legacy.encode_terminal_key(key.clone(), KeyboardProtocol::Legacy),
        b"\t"
    );

    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>3u");
    let kitty = PaneTerminal::new(terminal);
    assert_eq!(
        kitty.encode_terminal_key(key, KeyboardProtocol::Kitty { flags: 3 }),
        b"\x1b[9;5u"
    );
}

#[test]
fn pane_enter_backspace_release_in_legacy_pane_emits_nothing() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    for code in [ClientKeyCode::Enter, ClientKeyCode::Backspace] {
        let press = pane.encode_terminal_key(
            TerminalKey::new(code.clone(), KeyModifiers::NONE),
            KeyboardProtocol::Legacy,
        );
        let release = pane.encode_terminal_key(
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Release),
            KeyboardProtocol::Legacy,
        );
        assert!(!press.is_empty(), "{code:?} press should emit bytes");
        assert!(
            release.is_empty(),
            "{code:?} release should emit nothing in a legacy pane, got {release:?}"
        );
    }
}

#[test]
fn pane_report_event_pane_keeps_basic_compatibility_keys_legacy() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>3u");
    let pane = PaneTerminal::new(terminal);

    for (code, expected) in [
        (ClientKeyCode::Enter, b"\r".as_slice()),
        (ClientKeyCode::Backspace, b"\x7f".as_slice()),
    ] {
        let press = pane.encode_terminal_key(
            TerminalKey::new(code.clone(), KeyModifiers::NONE),
            pane.keyboard_protocol(KeyboardProtocol::Legacy),
        );
        assert_eq!(
            press, expected,
            "{code:?} press should stay legacy-compatible without REPORT_ALL_KEYS"
        );

        let release = pane.encode_terminal_key(
            TerminalKey::new(code.clone(), KeyModifiers::NONE).with_kind(ClientKeyKind::Release),
            pane.keyboard_protocol(KeyboardProtocol::Legacy),
        );
        assert!(
            release.is_empty(),
            "{code:?} release should not fall back to legacy bytes, got {release:?}"
        );
    }
}

#[test]
fn pane_char_keys_still_use_semantic_encoding() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>1u");
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_terminal_key(
        TerminalKey::new(
            ClientKeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        KeyboardProtocol::Legacy,
    );

    assert_eq!(encoded, vec![1]);
}

#[test]
fn pane_key_encoding_honors_application_cursor_mode() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.modes.set(maestro_terminal::Mode::CursorKeys, true);
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );

    assert_eq!(encoded, b"\x1bOA");
}

#[test]
fn pane_seed_handoff_input_state_restores_input_modes() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    pane.seed_handoff_input_state(InputState {
        alternate_screen: InputMode::from(true),
        application_cursor: InputMode::from(true),
        bracketed_paste: InputMode::from(true),
        focus_reporting: InputMode::from(true),
        mouse_protocol_mode: crate::input::MouseProtocolMode::ButtonMotion,
        mouse_protocol_encoding: crate::input::MouseProtocolEncoding::Sgr,
        mouse_alternate_scroll: InputMode::from(true),
        modify_other_keys: InputMode::from(true),
        color_scheme_reporting: InputMode::from(true),
    });

    assert_eq!(
        pane.input_state(),
        Some(InputState {
            alternate_screen: InputMode::from(true),
            application_cursor: InputMode::from(true),
            bracketed_paste: InputMode::from(true),
            focus_reporting: InputMode::from(true),
            mouse_protocol_mode: crate::input::MouseProtocolMode::ButtonMotion,
            mouse_protocol_encoding: crate::input::MouseProtocolEncoding::Sgr,
            mouse_alternate_scroll: InputMode::from(true),
            modify_other_keys: InputMode::from(true),
            color_scheme_reporting: InputMode::from(true),
        })
    );

    let encoded = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );
    assert_eq!(encoded, b"\x1bOA");

    let key = crate::input::parse_terminal_key_sequence("\x1b[13;2u").unwrap();
    let encoded = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);
    assert_eq!(encoded, b"\x1b[27;2;13~");
}

#[test]
fn pane_grouped_semantic_key_repeats_expand_at_the_destination() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let key = TerminalKey::new(ClientKeyCode::Char('x'), KeyModifiers::NONE).with_repeat_count(3);

    assert_eq!(
        pane.encode_terminal_key(key, KeyboardProtocol::Legacy),
        b"xxx"
    );
}

#[test]
fn pane_grouped_release_is_encoded_once() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[>11u");
    let pane = PaneTerminal::new(terminal);
    let protocol = pane.keyboard_protocol(KeyboardProtocol::Legacy);
    let release =
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE).with_kind(ClientKeyKind::Release);
    let expected = pane.encode_terminal_key(release.clone(), protocol);

    assert!(!expected.is_empty());
    let mut malformed_release = release;
    malformed_release.repeat_count = 3;
    assert_eq!(
        pane.encode_terminal_key(malformed_release, protocol),
        expected
    );
}

#[test]
fn pane_key_encoder_updates_after_terminal_mode_changes() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    let before = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );
    assert_eq!(before, b"\x1b[A");

    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[?1h").unwrap());

    let after = pane.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );
    assert_eq!(after, b"\x1bOA");
}

#[test]
fn pane_key_encoder_updates_after_kitty_flag_changes() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    let key = TerminalKey::new(
        ClientKeyCode::Enter,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );

    let before = pane.encode_terminal_key(key.clone(), KeyboardProtocol::Legacy);
    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[>1u").unwrap());
    let after = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);

    assert_ne!(before, after);
    assert_eq!(after, b"\x1b[13;6u");
}

#[test]
fn pane_kitty_pane_encodes_shift_enter_as_csi_u() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[>5u").unwrap());

    let key = crate::input::parse_terminal_key_sequence("\x1b[13;2u").unwrap();
    let encoded = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);

    assert_eq!(
        pane.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 5 }
    );
    assert_eq!(encoded, b"\x1b[13;2u");
}

#[test]
fn pane_seed_keyboard_protocol_flags_restores_shift_enter_encoding() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    pane.seed_keyboard_protocol_flags(5);

    let key = crate::input::parse_terminal_key_sequence("\x1b[13;2u").unwrap();
    let encoded = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);

    assert_eq!(
        pane.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 5 }
    );
    assert_eq!(encoded, b"\x1b[13;2u");
}

#[test]
fn pane_keyboard_protocol_state_replays_nested_stack() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[>1u\x1b[>5u").unwrap());

    let ansi = pane.kitty_keyboard_state_ansi().unwrap();

    let restored_terminal = Terminal::new(80, 24, 0).unwrap();
    let restored = PaneTerminal::new(restored_terminal);
    restored.seed_keyboard_protocol_ansi(&ansi);
    assert_eq!(
        restored.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 5 }
    );

    restored.seed_history_ansi(std::str::from_utf8(b"\x1b[<u").unwrap());
    assert_eq!(
        restored.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 1 }
    );
}

#[test]
fn pane_modify_other_keys_mode_one_preserves_shift_enter() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let key = crate::input::parse_terminal_key_sequence("\x1b[13;2u").unwrap();

    pane.seed_history_ansi("\x1b[>4;1m");
    let encoded = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);

    assert_eq!(encoded, b"\x1b[27;2;13~");
}

#[test]
fn pane_kitty_pane_encodes_parsed_legacy_alt_backspace_as_csi_u() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[>1u").unwrap());

    let key = crate::input::parse_terminal_key_sequence("\x1b\x7f").unwrap();
    let encoded = pane.encode_terminal_key(key, KeyboardProtocol::Legacy);

    assert_eq!(encoded, b"\x1b[127;3u");
}

#[test]
fn pane_kitty_pane_preserves_legacy_ctrl_alt_letter() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);

    pane.seed_history_ansi(std::str::from_utf8(b"\x1b[>5u").unwrap());

    let mut events = crate::input::parse_raw_input_bytes_sync(b"\x1b\x06");
    let crate::input::RawInputEvent::Key(key) = events.remove(0) else {
        panic!("expected key event");
    };
    let encoded = pane.encode_terminal_key(key, pane.keyboard_protocol(KeyboardProtocol::Legacy));

    assert_eq!(encoded, b"\x1b[102;7u");
}

#[test]
fn pane_key_encoders_are_isolated_per_pane() {
    let first = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());
    let second = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());

    first.seed_history_ansi("\x1b[?1h");

    let first_encoded = first.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );
    let second_encoded = second.encode_terminal_key(
        TerminalKey::new(ClientKeyCode::Up, KeyModifiers::NONE),
        KeyboardProtocol::Legacy,
    );

    assert_eq!(first_encoded, b"\x1bOA");
    assert_eq!(second_encoded, b"\x1b[A");
}

#[test]
fn pane_mouse_button_encoding_uses_live_terminal_state() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1000h\x1b[?1006h");
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_mouse_button(
        ClientMouseKind::Up(ClientMouseButton::Left),
        crate::input::mouse::Position::Cell { column: 11, row: 9 },
        KeyModifiers::NONE,
    );

    assert_eq!(encoded.as_deref(), Some(&b"\x1b[<0;12;10m"[..]));
}

#[test]
fn pane_mouse_drag_encoding_uses_motion_reporting_state() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1002h\x1b[?1006h");
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_mouse_button(
        ClientMouseKind::Drag(ClientMouseButton::Left),
        crate::input::mouse::Position::Cell { column: 4, row: 6 },
        KeyModifiers::SHIFT,
    );

    assert_eq!(encoded.as_deref(), Some(&b"\x1b[<36;5;7M"[..]));
}

#[test]
fn pane_mouse_drag_without_motion_reporting_is_not_forwarded() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1000h\x1b[?1006h");
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_mouse_button(
        ClientMouseKind::Drag(ClientMouseButton::Left),
        crate::input::mouse::Position::Cell { column: 4, row: 6 },
        KeyModifiers::NONE,
    );

    assert_eq!(encoded, None);
}

#[test]
fn pane_mouse_moved_encoding_uses_any_motion_state() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.write(b"\x1b[?1003h\x1b[?1006h");
    let pane = PaneTerminal::new(terminal);

    let encoded = pane.encode_mouse_motion(
        ClientMouseKind::Moved,
        crate::input::mouse::Position::Cell { column: 4, row: 6 },
        KeyModifiers::NONE,
    );

    assert_eq!(encoded.as_deref(), Some(&b"\x1b[<35;5;7M"[..]));
}

#[test]
fn pane_mouse_sgr_pixels_preserves_exact_and_downgrades_cell_input() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.resize(80, 24, 10, 20).unwrap();
    terminal.write(b"\x1b[?1003h\x1b[?1006h\x1b[?1016h");
    let pane = PaneTerminal::new(terminal);

    let exact = pane.encode_mouse_motion(
        ClientMouseKind::Moved,
        crate::input::mouse::Position::Pixels { x: 48, y: 139 },
        KeyModifiers::NONE,
    );
    let fallback = pane.encode_mouse_motion(
        ClientMouseKind::Moved,
        crate::input::mouse::Position::Cell { column: 4, row: 6 },
        KeyModifiers::NONE,
    );

    assert_eq!(exact.as_deref(), Some(&b"\x1b[<35;48;139M"[..]));
    assert_eq!(fallback.as_deref(), Some(&b"\x1b[<35;5;7M"[..]));
}

#[test]
fn pane_pixel_release_does_not_truncate_coordinates_above_u16() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.resize(80, 24, 10, 20).unwrap();
    terminal.write(b"\x1b[?1000h\x1b[?1016h");
    let pane = PaneTerminal::new(terminal);
    assert_eq!(
        pane.encode_mouse_button(
            ClientMouseKind::Up(ClientMouseButton::Left),
            crate::input::mouse::Position::Pixels { x: 70_000, y: 139 },
            KeyModifiers::NONE
        ),
        Some(b"\x1b[<0;70000;139m".to_vec())
    );
}

#[test]
fn pane_keyboard_flags_compose_split_sequences_without_rescanning() {
    let pane = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());
    pane.seed_history_ansi("\x1b[>1u\x1b[>4;");
    pane.seed_history_ansi("01m\x1b[>5u\x1b[<");
    pane.seed_history_ansi("u");
    assert_eq!(
        pane.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Kitty { flags: 1 }
    );
    assert_eq!(
        pane.kitty_keyboard_state_ansi().as_deref(),
        Some("\x1b[>1u")
    );
}

#[test]
fn pane_relocated_legacy_parser_encoding_assertions() {
    use crate::{input::parse_terminal_key_sequence, pane::input::encode_terminal_key};
    for (input, expected) in [
        ("\x1bA", b"\x1bA".as_slice()),
        ("\x1b\x06", b"\x1b\x06"),
        ("\n", b"\n"),
    ] {
        let key = parse_terminal_key_sequence(input).unwrap();
        assert_eq!(
            encode_terminal_key(&key, KeyboardProtocol::Legacy),
            expected
        );
    }
}

#[test]
fn pane_generated_text_and_unsupported_keys_use_semantic_fallback() {
    let pane = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());
    let key = TerminalKey::new(ClientKeyCode::Char('a'), KeyModifiers::NONE)
        .with_generated_text(Some("committed".to_owned()))
        .with_repeat_count(2);
    assert_eq!(
        pane.encode_terminal_key(key, KeyboardProtocol::Legacy),
        b"committedcommitted"
    );
    for code in [
        ClientKeyCode::Null,
        ClientKeyCode::F(0),
        ClientKeyCode::F(13),
    ] {
        assert!(
            pane.encode_terminal_key(
                TerminalKey::new(code, KeyModifiers::NONE),
                KeyboardProtocol::Legacy
            )
            .is_empty()
        );
    }
    for (ch, expected) in [
        ('i', 9),
        ('j', 10),
        ('m', 13),
        ('h', 8),
        ('[', 27),
        ('-', 31),
    ] {
        let key = TerminalKey::new(
            ClientKeyCode::Char(ch),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            pane.encode_terminal_key(key, KeyboardProtocol::Legacy),
            [expected]
        );
    }
}

#[test]
fn pane_legacy_alt_unicode_keeps_the_entire_text() {
    let pane = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());
    let key = TerminalKey::new(ClientKeyCode::Char('中'), KeyModifiers::ALT);
    assert_eq!(
        pane.encode_terminal_key(key, KeyboardProtocol::Legacy),
        "\x1b中".as_bytes()
    );
}

#[test]
fn pane_legacy_control_unicode_uses_valid_protocol_not_a_truncated_byte() {
    let pane = PaneTerminal::new(Terminal::new(80, 24, 0).unwrap());
    let unicode = TerminalKey::new(ClientKeyCode::Char('中'), KeyModifiers::CONTROL);
    let bytes = pane.encode_terminal_key(unicode, KeyboardProtocol::Legacy);
    assert_eq!(bytes, b"\x1b[20013;5u");
    assert_ne!(bytes, [0x2d]);
    let ascii = TerminalKey::new(ClientKeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        pane.encode_terminal_key(ascii, KeyboardProtocol::Legacy),
        [3]
    );
}

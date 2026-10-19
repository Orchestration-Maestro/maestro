//! Shared host input recognition boundaries.

use maestro_tui::stdin_buffer::scan_sgr_mouse_fields;

#[test]
fn sgr_field_scanner_preserves_each_consumer_domain() {
    use crate::input::mouse::parse_report;

    assert_host_mouse_admission();
    assert_host_mouse_classes();

    for (data, geometry) in [
        (b"\x1b[<0;1;2M".as_slice(), Some((1, 2))),
        (b"\x1b[<0;1;2m", Some((1, 2))),
        (b"\x1b[<0;0;0M", Some((0, 0))),
        (b"\x1b[<256;65536;4294967295M", Some((65536, u32::MAX))),
        (b"\x1b[<0;65535;65535M", Some((65535, 65535))),
        (b"\x1b[<0;1;2;9M", None),
        (b"\x1b[<0;1;2;M", None),
        (b"\x1b[<+0;+1;+2M", None),
        (b"\x1b[<0;;2M", None),
        (b"\x1b[<0;4294967296;2M", None),
        (b"\x1b[<255;1;2M", Some((1, 2))),
    ] {
        assert!(scan_sgr_mouse_fields(data).is_some());
        assert_eq!(parse_report(data), geometry);
    }
    let fields = scan_sgr_mouse_fields(b"\x1b[<+0;;4294967296;9;10m").unwrap();
    assert_eq!(fields.cb, b"+0");
    assert_eq!(fields.column, b"");
    assert_eq!(fields.row, b"4294967296");
    assert_eq!(fields.extra, Some(b"9;10".as_slice()));
    assert!(fields.release);
    let fields = scan_sgr_mouse_fields(b"\x1b[<0;1;2M").unwrap();
    assert_eq!(fields.extra, None);
    assert!(!fields.release);
    for malformed in [
        b"\x1b[<0;1M".as_slice(),
        b"\x1b[<0;1;2X",
        b"\x1b[<0;1;2Mkey",
    ] {
        assert!(scan_sgr_mouse_fields(malformed).is_none());
    }
}

#[test]
fn structured_modifier_projection_covers_all_bit_combinations() {
    use maestro_protocol::frames::KeyModifiers;
    for input in 0u32..=255 {
        let data = format!("\x1b[97;{}u", input + 1);
        // The host report encodes modifiers in a nonzero byte.
        if input == 255 {
            assert!(crate::input::parse_terminal_key_sequence(&data).is_none());
            continue;
        }
        let wire = crate::input::parse_terminal_key_sequence(&data)
            .unwrap()
            .modifiers;
        assert_eq!(wire & KeyModifiers::ALT != 0, input & 2 != 0);
        assert_eq!(wire & KeyModifiers::CONTROL != 0, input & 4 != 0);
        assert_eq!(
            u32::from(wire & (1 | 8 | 16 | 32)),
            input & (1 | 8 | 16 | 32)
        );
        let restored = u32::from(
            (wire & (1 | 8 | 16 | 32))
                | ((wire & KeyModifiers::ALT) >> 1)
                | ((wire & KeyModifiers::CONTROL) << 1),
        );
        assert_eq!(restored, input & 63);
    }
}

#[test]
fn native_functional_keys_are_filtered_only_at_projection() {
    use maestro_protocol::frames::ClientKeyCode;
    use maestro_tui::keys;
    for point in 57344..=57454 {
        let data = format!("\x1b[{point}u");
        let native = keys::parse_terminal_key_sequence(&data);
        let wire = crate::input::parse_terminal_key_sequence(&data);
        if let Some(native) = native {
            let supported = matches!(
                native.code,
                keys::Code::Char(_)
                    | keys::Code::Key(
                        keys::Named::Escape
                            | keys::Named::Enter
                            | keys::Named::Tab
                            | keys::Named::Backspace
                            | keys::Named::Insert
                            | keys::Named::Delete
                            | keys::Named::Left
                            | keys::Named::Right
                            | keys::Named::Up
                            | keys::Named::Down
                            | keys::Named::PageUp
                            | keys::Named::PageDown
                            | keys::Named::Home
                            | keys::Named::End
                            | keys::Named::Function(_)
                    )
            );
            assert_eq!(wire.is_some(), supported, "{point}");
            if (57376..=57398).contains(&point) {
                assert_eq!(
                    wire.unwrap().code,
                    ClientKeyCode::F(u8::try_from(point - 57363).unwrap())
                );
            }
        } else {
            assert!(wire.is_none(), "{point}");
        }
    }
    assert!(crate::input::parse_terminal_key_sequence("\x1b[57364u").is_none());
}

use crate::input::raw_input::{
    RawInputFramer, events_require_host_surface_redraw,
    events_require_host_terminal_appearance_query, events_require_host_terminal_theme_query,
    extract_one_event, input_flush_timeout, is_complete_text_bracketed_paste,
};
use crate::input::{RawInputEvent, parse_raw_input_bytes_sync};
use crate::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
use maestro_protocol::frames::{
    ClientKeyCode as KeyCode, ClientKeyKind as KeyEventKind, ClientMouseButton as MouseButton,
    ClientMouseKind as MouseEventKind, KeyModifiers,
};
use maestro_tui::stdin_buffer::RawInputByteFramer;

/// Dark host appearance bytes.
const DARK_REPORT: &[u8] = b"\x1b[?997;1n";
/// Light host appearance bytes.
const LIGHT_REPORT: &[u8] = b"\x1b[?997;2n";

/// Check an owned projected key.
fn assert_raw_key(event: RawInputEvent, code: &KeyCode, modifiers: u8) {
    let RawInputEvent::Key(key) = event else {
        panic!("expected key");
    };
    assert_eq!(&key.code, code);
    assert_eq!(key.modifiers, modifiers);
}

/// Event with its original byte range, used only by source assertions.
struct RangedEvent {
    /// Decoded event.
    event: RawInputEvent,
    /// Starting offset.
    start: usize,
    /// Consumed bytes.
    len: usize,
}

/// Record consumed ranges through the real projection.
fn parse_raw_input_bytes_with_ranges(data: &[u8]) -> Vec<RangedEvent> {
    let mut events = Vec::new();
    let mut start = 0;
    while let Some((event, len)) = extract_one_event(&data[start..]) {
        events.push(RangedEvent { event, start, len });
        start += len;
    }
    events
}
#[test]
fn host_decodes_extended_shift_letter_release() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[108:76;2:3u").unwrap()
    else {
        panic!("expected key");
    };
    assert_eq!(consumed, 13);
    assert_eq!(key.code, KeyCode::Char('l'));
    assert_eq!(key.modifiers, KeyModifiers::SHIFT);
    assert_eq!(key.kind, KeyEventKind::Release);
    assert_eq!(key.shifted_codepoint, Some('L' as u32));
}

#[test]
fn host_decodes_bracketed_paste() {
    let (RawInputEvent::Paste(text), consumed) =
        extract_one_event(b"\x1b[200~hello\x1b[201~rest").unwrap()
    else {
        panic!("expected paste");
    };
    assert_eq!(text, "hello");
    assert_eq!(consumed, 17);
}

#[test]
fn host_complete_text_bracketed_paste_requires_one_exact_utf8_sequence() {
    assert!(is_complete_text_bracketed_paste(b"\x1b[200~hello\x1b[201~"));
    assert!(!is_complete_text_bracketed_paste(b"\x1b[200~hello"));
    assert!(!is_complete_text_bracketed_paste(
        b"\x1b[200~hello\x1b[201~rest"
    ));
    assert!(!is_complete_text_bracketed_paste(
        b"\x1b[200~one\x1b[201~\x1b[200~two\x1b[201~"
    ));
    assert!(!is_complete_text_bracketed_paste(b"\x1b[200~\xff\x1b[201~"));
}

#[test]
fn host_decodes_sgr_mouse() {
    let (
        RawInputEvent::Mouse {
            kind,
            column,
            row,
            modifiers,
        },
        consumed,
    ) = extract_one_event(b"\x1b[<0;20;10M").unwrap()
    else {
        panic!("expected mouse");
    };
    assert_eq!(consumed, 11);
    assert_eq!(kind, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(column, 19);
    assert_eq!(row, 9);
    assert_eq!(modifiers, KeyModifiers::NONE);
}

#[test]
fn host_decodes_default_mouse_encoding() {
    let events = parse_raw_input_bytes_sync(b"\x1b[MCN1");
    let [
        RawInputEvent::Mouse {
            kind,
            column,
            row,
            modifiers,
        },
    ] = events.as_slice()
    else {
        panic!("expected one mouse event");
    };
    assert_eq!(*kind, MouseEventKind::Moved);
    assert_eq!((*column, *row), (45, 16));
    assert_eq!(*modifiers, KeyModifiers::NONE);
}

#[test]
fn host_rejected_default_mouse_frame_preserves_trailing_input() {
    let events = parse_raw_input_bytes_with_ranges(b"\x1b[M\x82AAx");

    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].event, RawInputEvent::Unsupported));
    assert_eq!((events[0].start, events[0].len), (0, 6));
    assert!(matches!(events[1].event, RawInputEvent::Key(_)));
    assert_eq!((events[1].start, events[1].len), (6, 1));
}

#[test]
fn host_decodes_extended_button_drag_as_mouse_motion() {
    for input in [
        b"\x1b[<160;20;10M".as_slice(),
        b"\x1b[<161;20;10M".as_slice(),
    ] {
        let (
            RawInputEvent::Mouse {
                kind, column, row, ..
            },
            _,
        ) = extract_one_event(input).unwrap()
        else {
            panic!("expected mouse");
        };
        assert_eq!(kind, MouseEventKind::Moved);
        assert_eq!((column, row), (19, 9));
    }
}

#[test]
fn host_decodes_sgr_mouse_observable_modifiers() {
    let cases = [
        (b"\x1b[<8;20;10M".as_slice(), KeyModifiers::ALT),
        (b"\x1b[<16;20;10M".as_slice(), KeyModifiers::CONTROL),
        (
            b"\x1b[<24;20;10M".as_slice(),
            KeyModifiers::ALT | KeyModifiers::CONTROL,
        ),
    ];

    for (input, expected) in cases {
        let (RawInputEvent::Mouse { modifiers, .. }, _) = extract_one_event(input).unwrap() else {
            panic!("expected mouse");
        };
        assert_eq!(modifiers, expected);
        assert_eq!(modifiers & KeyModifiers::SUPER, 0);
    }
}

#[test]
fn host_decodes_host_default_color_response_with_st() {
    let (RawInputEvent::HostDefaultColor { kind, color }, consumed) =
        extract_one_event(b"\x1b]10;rgb:cccc/dddd/eeee\x1b\\").unwrap()
    else {
        panic!("expected host color response");
    };
    assert_eq!(consumed, 25);
    assert_eq!(kind, DefaultColorKind::Foreground);
    assert_eq!(
        color,
        RgbColor {
            r: 0xcc,
            g: 0xdd,
            b: 0xee
        }
    );
}

#[test]
fn host_decodes_host_default_color_response_with_bel() {
    let (RawInputEvent::HostDefaultColor { kind, color }, consumed) =
        extract_one_event(b"\x1b]11;#112233\x07").unwrap()
    else {
        panic!("expected host color response");
    };
    assert_eq!(consumed, 13);
    assert_eq!(kind, DefaultColorKind::Background);
    assert_eq!(
        color,
        RgbColor {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
}

#[test]
fn host_decodes_host_palette_color_response() {
    let (RawInputEvent::HostPaletteColors { colors }, consumed) =
        extract_one_event(b"\x1b]4;7;rgb:1111/2222/3333\x1b\\").unwrap()
    else {
        panic!("expected host palette response");
    };
    assert_eq!(consumed, 26);
    assert_eq!(
        colors,
        vec![(
            7,
            RgbColor {
                r: 0x11,
                g: 0x22,
                b: 0x33,
            }
        )]
    );
}

#[test]
fn host_decodes_legacy_up_arrow() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[A").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 3);
    assert_eq!(key.code, KeyCode::Up);
}

#[test]
fn host_decodes_outer_focus_events() {
    let (event, consumed) = extract_one_event(b"\x1b[I").unwrap();
    assert_eq!(consumed, 3);
    assert!(matches!(event, RawInputEvent::OuterFocusGained));

    let (event, consumed) = extract_one_event(b"\x1b[O").unwrap();
    assert_eq!(consumed, 3);
    assert!(matches!(event, RawInputEvent::OuterFocusLost));
}

#[test]
fn host_outer_focus_gained_requests_host_surface_redraw() {
    let events = parse_raw_input_bytes_sync(b"\x1b[I");
    assert!(events_require_host_surface_redraw(&events, true));
    assert!(!events_require_host_surface_redraw(&events, false));

    let events = parse_raw_input_bytes_sync(b"\x1b[O");
    assert!(!events_require_host_surface_redraw(&events, true));
}

#[test]
fn host_outer_focus_gained_requests_host_appearance_query() {
    let gained = parse_raw_input_bytes_sync(b"\x1b[I");
    let lost = parse_raw_input_bytes_sync(b"\x1b[O");
    let scheme_report = parse_raw_input_bytes_sync(b"\x1b[?997;1n");

    assert!(events_require_host_terminal_appearance_query(&gained));
    assert!(!events_require_host_terminal_appearance_query(&lost));
    assert!(!events_require_host_terminal_appearance_query(
        &scheme_report
    ));
    assert!(events_require_host_terminal_theme_query(&scheme_report));
}

#[test]
fn host_decodes_enhanced_color_scheme_reports() {
    for bytes in [DARK_REPORT, LIGHT_REPORT] {
        let events = parse_raw_input_bytes_sync(bytes);
        assert_eq!(events.len(), 1, "bytes: {bytes:?}");
        assert!(matches!(
            events[0],
            RawInputEvent::HostColorSchemeChanged(HostAppearance::Dark | HostAppearance::Light)
        ));
        assert!(events_require_host_terminal_theme_query(&events));
    }
}

#[test]
fn host_enhanced_color_scheme_report_parser_is_exact() {
    for bytes in [
        b"\x1b[?997;0n".as_slice(),
        b"\x1b[?997;3n".as_slice(),
        b"\x1b[?998;1n".as_slice(),
    ] {
        let events = parse_raw_input_bytes_sync(bytes);
        assert_eq!(events.len(), 1, "bytes: {bytes:?}");
        assert!(matches!(events[0], RawInputEvent::Unsupported));
        assert!(!events_require_host_terminal_theme_query(&events));
    }
}

#[test]
fn host_input_framer_reassembles_split_color_scheme_report() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b[?997;").is_empty());
    let events = framer.push(b"1n");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::HostColorSchemeChanged(HostAppearance::Dark)
    ));
}

#[test]
fn host_decodes_host_cell_size_report() {
    let events = parse_raw_input_bytes_sync(b"\x1b[6;21;10t");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::HostCellSizeReport {
            width_px: 10,
            height_px: 21,
        }
    ));
}

#[test]
fn host_decodes_xterm_alt_up_arrow() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[1;3A").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 6);
    assert_eq!(key.code, KeyCode::Up);
    assert_eq!(key.modifiers, KeyModifiers::ALT);
}

#[test]
fn host_decodes_legacy_alt_backspace() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b\x7f").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 2);
    assert_eq!(key.code, KeyCode::Backspace);
    assert_eq!(key.modifiers, KeyModifiers::ALT);
}

#[test]
fn host_decodes_extended_alt_backspace() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[127;3u").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 8);
    assert_eq!(key.code, KeyCode::Backspace);
    assert_eq!(key.modifiers, KeyModifiers::ALT);
}

#[test]
fn host_decodes_enhanced_pageup_press() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[5;1:1~").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 8);
    assert_eq!(key.code, KeyCode::PageUp);
    assert_eq!(key.modifiers, KeyModifiers::NONE);
    assert_eq!(key.kind, KeyEventKind::Press);
}

#[test]
fn host_decodes_enhanced_pagedown_release() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x1b[6;1:3~").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 8);
    assert_eq!(key.code, KeyCode::PageDown);
    assert_eq!(key.modifiers, KeyModifiers::NONE);
    assert_eq!(key.kind, KeyEventKind::Release);
}

#[test]
fn host_input_family_matrix_is_covered() {
    let cases: &[(&[u8], KeyCode, u8)] = &[
        (b"\x02", KeyCode::Char('b'), KeyModifiers::CONTROL),
        (b"\r", KeyCode::Enter, KeyModifiers::NONE),
        (b"\t", KeyCode::Tab, KeyModifiers::NONE),
        (b"\x7f", KeyCode::Backspace, KeyModifiers::NONE),
        (b"\x1b[A", KeyCode::Up, KeyModifiers::NONE),
        (b"\x1b[1;3A", KeyCode::Up, KeyModifiers::ALT),
        (b"\x1b\x7f", KeyCode::Backspace, KeyModifiers::ALT),
        (b"\x1b[127;3u", KeyCode::Backspace, KeyModifiers::ALT),
        (b"\x1b[57420;1u", KeyCode::Down, KeyModifiers::NONE),
        (b"\x1b[57423;1u", KeyCode::Home, KeyModifiers::NONE),
        (b"\x1bOq", KeyCode::Char('1'), KeyModifiers::NONE),
        (b"\x1b[14~", KeyCode::F(4), KeyModifiers::NONE),
        (b"\x1b[49:33;2:1u", KeyCode::Char('1'), KeyModifiers::SHIFT),
    ];

    for (bytes, code, modifiers) in cases {
        let (event, consumed) = extract_one_event(bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_raw_key(event, code, *modifiers);
    }
}

#[test]
fn host_raw_framer_waits_for_application_keypad_sequence_final_byte() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1bO").is_empty());
    let events = framer.push(b"q");

    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('1'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_unsupported_ss3_sequence_stays_unsupported() {
    let (event, consumed) = extract_one_event(b"\x1bOz").unwrap();

    assert_eq!(consumed, 3);
    assert!(matches!(event, RawInputEvent::Unsupported));
}

#[test]
fn host_modified_rxvt_f_key_alias_stays_unsupported() {
    let (event, consumed) = extract_one_event(b"\x1b[14;3~").unwrap();

    assert_eq!(consumed, 7);
    assert!(matches!(event, RawInputEvent::Unsupported));
}

#[test]
fn host_flushes_lone_escape_after_timeout() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();
    emitted.extend(framer.push(&[27]));
    emitted.extend(framer.flush_timeout());
    assert!(!framer.has_pending_input());
    let event = emitted.remove(0);
    let RawInputEvent::Key(key) = event else {
        panic!("expected key");
    };
    assert_eq!(key.code, KeyCode::Esc);
}

#[test]
fn host_decodes_raw_ctrl_b() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\x02").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 1);
    assert_eq!(key.code, KeyCode::Char('b'));
    assert_eq!(key.modifiers, KeyModifiers::CONTROL);
}

#[test]
fn host_decodes_raw_lf_as_ctrl_j() {
    let (RawInputEvent::Key(key), consumed) = extract_one_event(b"\n").unwrap() else {
        panic!("expected key");
    };
    assert_eq!(consumed, 1);
    assert_eq!(key.code, KeyCode::Char('j'));
    assert_eq!(key.modifiers, KeyModifiers::CONTROL);
}

#[test]
fn host_input_corpus_fixture_extracts_whole_events() {
    let corpus =
        include_str!("../../../../maestro-tui/tests/fixtures/host_input/keyboard_reports.tsv");
    assert_fixture_extracts_whole_events(corpus, false);
}

#[test]
fn host_input_macos_terminal_variants_fixture_extracts_whole_events() {
    let corpus =
        include_str!("../../../../maestro-tui/tests/fixtures/host_input/macos_key_samples.tsv");
    assert_fixture_extracts_whole_events(corpus, true);
}

#[test]
fn host_input_linux_terminal_variants_fixture_extracts_whole_events() {
    let corpus =
        include_str!("../../../../maestro-tui/tests/fixtures/host_input/linux_key_samples.tsv");
    assert_fixture_extracts_whole_events(corpus, false);
}

#[test]
fn host_chunked_legacy_arrow_waits_for_completion() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b"));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"[A"));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Up,
        KeyModifiers::NONE,
    );
}

#[test]
fn host_lone_escape_is_buffered_until_timeout_flush() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b"));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.flush_timeout());
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Esc,
        KeyModifiers::NONE,
    );
}

#[test]
fn host_escape_followed_by_arrow_before_flush_does_not_emit_escape() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b"));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"[B"));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Down,
        KeyModifiers::NONE,
    );
}

#[test]
fn host_escape_followed_by_sgr_mouse_before_flush_does_not_emit_text() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    let events = framer.push(b"[<65;43;26M");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::Mouse {
            kind: MouseEventKind::ScrollDown,
            column: 42,
            row: 25,
            ..
        }
    ));
}

#[test]
fn host_lone_escape_then_complete_sgr_mouse_report_emits_both_events() {
    for report in [b"\x1b[<35;10;20M".as_slice(), b"\x1b[<35;10;20m".as_slice()] {
        let mut framer = RawInputFramer::default();

        assert!(framer.push(b"\x1b").is_empty());
        let events = framer.push(report);

        assert_eq!(events.len(), 2);
        let mut events = events.into_iter();
        assert_raw_key(events.next().unwrap(), &KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches!(
            events.next().unwrap(),
            RawInputEvent::Mouse {
                kind: MouseEventKind::Moved,
                column: 9,
                row: 19,
                ..
            }
        ));
        assert!(framer.flush_timeout().is_empty());
    }
}

#[test]
fn host_lone_escape_then_default_mouse_report_emits_both_events() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    let events = framer.push(b"\x1b[MCN1");

    assert_eq!(events.len(), 2);
    let mut events = events.into_iter();
    assert_raw_key(events.next().unwrap(), &KeyCode::Esc, KeyModifiers::NONE);
    assert!(matches!(
        events.next().unwrap(),
        RawInputEvent::Mouse {
            kind: MouseEventKind::Moved,
            column: 45,
            row: 16,
            ..
        }
    ));
    assert!(framer.flush_timeout().is_empty());
}

#[test]
fn host_legacy_doubled_escape_alt_arrow_remains_one_event() {
    let mut framer = RawInputFramer::default();

    let events = framer.push(b"\x1b\x1b[A");

    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Up,
        KeyModifiers::ALT,
    );
    assert!(framer.flush_timeout().is_empty());
}

#[test]
fn host_legacy_reader_extends_incomplete_mouse_timeouts() {
    let mut sgr_mouse = RawInputFramer::default();
    assert!(sgr_mouse.push(b"\x1b[<3").is_empty());
    assert_eq!(
        input_flush_timeout(&sgr_mouse),
        std::time::Duration::from_millis(150)
    );

    let report = b"\x1b[MCN1";
    for split in 3..report.len() {
        let mut default_mouse = RawInputFramer::default();
        assert!(default_mouse.push(&report[..split]).is_empty());
        assert_eq!(
            input_flush_timeout(&default_mouse),
            std::time::Duration::from_millis(150)
        );
        assert!(matches!(
            default_mouse.push(&report[split..]).as_slice(),
            [RawInputEvent::Mouse { .. }]
        ));
    }

    let mut escape = RawInputFramer::default();
    assert!(escape.push(b"\x1b").is_empty());
    assert_eq!(
        input_flush_timeout(&escape),
        std::time::Duration::from_millis(10)
    );
}

#[test]
fn host_sgr_mouse_sequence_split_after_button_prefix_is_reassembled_before_timeout() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b[<3").is_empty());
    let events = framer.push(b"5;58;30M");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::Mouse {
            kind: MouseEventKind::Moved,
            column: 57,
            row: 29,
            ..
        }
    ));
}

#[test]
fn host_timed_out_split_sgr_mouse_tail_is_discarded_and_following_input_is_preserved() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b[<3").is_empty());
    assert!(framer.flush_timeout().is_empty());
    let events = framer.push(b"5;58;30Mx");

    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('x'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_sgr_mouse_tail_after_lone_escape_timeout_is_discarded() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    let timeout_events = framer.flush_timeout();
    assert_eq!(timeout_events.len(), 1);
    assert_raw_key(
        timeout_events.into_iter().next().unwrap(),
        &KeyCode::Esc,
        KeyModifiers::NONE,
    );

    assert!(framer.push(b"[<65;43;26M").is_empty());
}

#[test]
fn host_input_after_discarded_complete_sgr_mouse_tail_is_preserved() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout().len(), 1);
    let events = framer.push(b"[<65;43;26Mx");
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('x'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_invalid_orphaned_sgr_mouse_tail_after_escape_timeout_is_preserved() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout().len(), 1);

    let events = framer.push(b"[<x");

    assert_eq!(events.len(), 3);
    assert_raw_key(
        events.into_iter().last().unwrap(),
        &KeyCode::Char('x'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_double_split_sgr_mouse_tail_after_lone_escape_timeout_is_discarded() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b").is_empty());
    assert_eq!(framer.flush_timeout().len(), 1);

    assert!(framer.push(b"[<65;4").is_empty());
    assert!(framer.flush_timeout().is_empty());
    let events = framer.push(b"3;26Mx");
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('x'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_escape_followed_by_alt_char_before_flush_becomes_alt_key() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b"));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"b"));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('b'),
        KeyModifiers::ALT,
    );
}

#[test]
fn host_chunked_extended_sequence_waits_for_completion() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b[49:33;2:"));
    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"1u"));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('1'),
        KeyModifiers::SHIFT,
    );
}

#[test]
fn host_chunked_bracketed_paste_waits_for_terminator() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b[200~hello"));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"\x1b[201~"));
    assert!(!framer.has_pending_input());
    assert!(!framer.has_pending_bracketed_paste());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    let RawInputEvent::Paste(text) = &events[0] else {
        panic!("expected paste");
    };
    assert_eq!(text, "hello");
}

#[test]
fn host_incomplete_bracketed_paste_is_not_flushed_on_timeout() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push(b"\x1b[200~hello\nworld"));

    assert!(std::mem::take(&mut emitted).is_empty());

    assert!(framer.has_pending_bracketed_paste());
    emitted.extend(framer.flush_timeout());

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(b"\x1b[201~"));
    assert!(!framer.has_pending_input());
    assert!(!framer.has_pending_bracketed_paste());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    let RawInputEvent::Paste(text) = &events[0] else {
        panic!("expected paste");
    };
    assert_eq!(text, "hello\nworld");
}

#[test]
fn host_alt_utf8_char_drains_as_one_event_before_following_input() {
    let events = parse_raw_input_bytes_sync("\x1béx".as_bytes());
    assert_eq!(events.len(), 2);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('é'),
        KeyModifiers::ALT,
    );
}

#[test]
fn host_chunked_alt_utf8_waits_for_continuation_byte_after_escape() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();
    let bytes = "\x1bé".as_bytes();

    emitted.extend(framer.push(&bytes[..2]));

    assert!(std::mem::take(&mut emitted).is_empty());
    emitted.extend(framer.flush_timeout());

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(&bytes[2..]));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('é'),
        KeyModifiers::ALT,
    );
}

#[test]
fn host_chunked_utf8_waits_for_continuation_byte() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();

    emitted.extend(framer.push("é".as_bytes().get(..1).unwrap()));

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push("é".as_bytes().get(1..).unwrap()));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('é'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_chunked_cjk_utf8_waits_for_all_continuation_bytes() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();
    let bytes = "好".as_bytes();

    emitted.extend(framer.push(&bytes[..1]));

    assert!(std::mem::take(&mut emitted).is_empty());
    emitted.extend(framer.flush_timeout());

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(&bytes[1..2]));

    assert!(std::mem::take(&mut emitted).is_empty());
    emitted.extend(framer.flush_timeout());

    assert!(std::mem::take(&mut emitted).is_empty());

    emitted.extend(framer.push(&bytes[2..]));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('好'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_chunked_four_byte_utf8_waits_for_all_continuation_bytes() {
    let mut emitted = Vec::new();
    let mut framer = RawInputFramer::default();
    let bytes = "🙂".as_bytes();

    for split in 1..bytes.len() {
        emitted.extend(framer.push(&bytes[split - 1..split]));

        assert!(std::mem::take(&mut emitted).is_empty());
        emitted.extend(framer.flush_timeout());

        assert!(std::mem::take(&mut emitted).is_empty());
    }

    emitted.extend(framer.push(&bytes[bytes.len() - 1..]));
    assert!(!framer.has_pending_input());
    let events = std::mem::take(&mut emitted);
    assert_eq!(events.len(), 1);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('🙂'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_parse_with_ranges_tracks_byte_offsets() {
    let input = b"\x1b[Aa\x1b[B".to_vec();
    let ranges = parse_raw_input_bytes_with_ranges(&input);

    assert_eq!(ranges.len(), 3, "should parse three events");

    assert_eq!(ranges[0].start, 0);
    assert_eq!(ranges[0].len, 3);
    assert_ranged_key(&ranges[0], &KeyCode::Up);

    assert_eq!(ranges[1].start, 3);
    assert_eq!(ranges[1].len, 1);
    assert!(matches!(
        &ranges[1].event,
        RawInputEvent::Key(k) if k.code == KeyCode::Char('a')
    ));

    assert_eq!(ranges[2].start, 4);
    assert_eq!(ranges[2].len, 3);
    assert!(matches!(
        &ranges[2].event,
        RawInputEvent::Key(k) if k.code == KeyCode::Down
    ));

    assert_source_ranges(&input, &ranges);
}

#[test]
fn host_parse_with_ranges_handles_single_event() {
    let input = b"a".to_vec();
    let ranges = parse_raw_input_bytes_with_ranges(&input);

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start, 0);
    assert_eq!(ranges[0].len, 1);
}

#[test]
fn host_parse_with_ranges_handles_mouse_event() {
    let input = b"\x1b[<0;20;10M".to_vec();
    let ranges = parse_raw_input_bytes_with_ranges(&input);

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start, 0);
    assert_eq!(ranges[0].len, input.len());
    assert!(matches!(&ranges[0].event, RawInputEvent::Mouse { .. }));
}

#[test]
fn host_decodes_enhanced_default_background_response() {
    let events = parse_raw_input_bytes_sync(b"\x1b]11;rgb:2828/2a2a/3636\x07");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Background,
            color: RgbColor {
                r: 0x28,
                g: 0x2a,
                b: 0x36
            }
        }
    ));
}

#[test]
fn host_input_framer_reassembles_split_default_background_response() {
    let mut framer = RawInputFramer::default();

    assert!(framer.push(b"\x1b]").is_empty());
    let events = framer.push(b"11;#123456\x07");

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Background,
            color: RgbColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            }
        }
    ));
}

#[test]
fn host_parse_input_bytes_sync_does_not_parse_incomplete_strings_as_alt_keys() {
    for bytes in [
        b"\x1b]".as_slice(),
        b"\x1bP".as_slice(),
        b"\x1b_".as_slice(),
        b"\x1b^".as_slice(),
        b"\x1bX".as_slice(),
    ] {
        let events = parse_raw_input_bytes_sync(bytes);

        assert!(events.is_empty(), "parsed {bytes:?} as {events:?}");
    }
}

#[test]
fn host_non_osc_control_strings_ignore_bel_and_complete_at_st() {
    let bytes = b"\x1bPabc\x07def\x1b\\x";

    let (event, consumed) = extract_one_event(bytes).unwrap();

    assert!(matches!(event, RawInputEvent::Unsupported));
    assert_eq!(consumed, b"\x1bPabc\x07def\x1b\\".len());
}

#[test]
fn host_non_osc_default_color_text_remains_key_input() {
    let events = parse_raw_input_bytes_sync(b"11;rgb:2828/2a2a/3636\x07");

    assert_eq!(events.len(), 22);
    assert_raw_key(
        events.into_iter().next().unwrap(),
        &KeyCode::Char('1'),
        KeyModifiers::NONE,
    );
}

#[test]
fn host_holds_lone_escape_and_stitches_split_host_color_reply() {
    let mut framer = RawInputByteFramer::default();
    framer.host_color_query_sent();

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());

    let chunks = framer.push(b"]11;rgb:2424/2727/3a3a\x1b\\");
    assert_eq!(chunks.len(), 1);
    let (event, _) = extract_one_event(&chunks[0]).unwrap();
    assert!(matches!(
        event,
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Background,
            ..
        }
    ));
}

#[test]
fn host_holds_lone_escape_and_stitches_split_host_cell_size_reply() {
    let mut framer = RawInputByteFramer::default();
    framer.host_cell_size_query_sent();

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());

    let chunks = framer.push(b"[6;21;10t");
    assert_eq!(chunks, vec![b"\x1b[6;21;10t".to_vec()]);
    let (event, _) = extract_one_event(&chunks[0]).unwrap();
    assert!(matches!(
        event,
        RawInputEvent::HostCellSizeReport {
            width_px: 10,
            height_px: 21,
        }
    ));
}

#[test]
fn host_opted_in_byte_framer_rearms_after_color_scheme_report() {
    let mut framer = RawInputByteFramer::default();
    framer.enable_host_color_scheme_change_tracking();

    assert_eq!(framer.push(DARK_REPORT), vec![DARK_REPORT.to_vec()]);

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    let chunks = framer.push(b"]10;#abcdef\x07");
    assert_eq!(chunks.len(), 1);
    let (event, _) = extract_one_event(&chunks[0]).unwrap();
    assert!(matches!(
        event,
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Foreground,
            color: RgbColor {
                r: 0xab,
                g: 0xcd,
                b: 0xef
            }
        }
    ));

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    let chunks = framer.push(b"]11;#123456\x07");
    assert_eq!(chunks.len(), 1);
    let (event, _) = extract_one_event(&chunks[0]).unwrap();
    assert!(matches!(
        event,
        RawInputEvent::HostDefaultColor {
            kind: DefaultColorKind::Background,
            color: RgbColor {
                r: 0x12,
                g: 0x34,
                b: 0x56
            }
        }
    ));

    assert!(framer.push(b"\x1b").is_empty());
    assert!(framer.flush_timeout().is_empty());
    assert_eq!(framer.flush_timeout(), vec![b"\x1b".to_vec()]);
}

/// Decode the source fixture bytes.
fn decode_hex(hex: &str) -> Vec<u8> {
    let hex = hex.trim();
    assert_eq!(hex.len() % 2, 0, "hex string must have even length");
    (0..hex.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&hex[idx..idx + 2], 16).unwrap())
        .collect()
}

/// Read a fixture key identity.
fn parse_fixture_key_code(value: &str) -> KeyCode {
    match value {
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "esc" => KeyCode::Esc,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "insert" => KeyCode::Insert,
        "delete" => KeyCode::Delete,
        value if value.starts_with("char:") => {
            KeyCode::Char(value.trim_start_matches("char:").chars().next().unwrap())
        }
        other => panic!("unsupported fixture key code: {other}"),
    }
}

/// Read the source fixture modifier spelling.
fn parse_fixture_modifiers(value: &str) -> u8 {
    if value == "-" || value.is_empty() {
        return KeyModifiers::NONE;
    }

    let mut modifiers = KeyModifiers::NONE;
    for part in value.split('+') {
        match part {
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" => modifiers |= KeyModifiers::ALT,
            "control" => modifiers |= KeyModifiers::CONTROL,
            "super" => modifiers |= KeyModifiers::SUPER,
            "hyper" => modifiers |= KeyModifiers::HYPER,
            "meta" => modifiers |= KeyModifiers::META,
            other => panic!("unsupported fixture modifier: {other}"),
        }
    }
    modifiers
}

/// Extract each fixture as one complete event without rewriting its fields.
fn assert_fixture_extracts_whole_events(corpus: &str, macos_layout: bool) {
    for line in corpus
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let mut columns: Vec<_> = line.split('\t').collect();
        if columns.len() == 5 {
            columns.push("");
        }
        if macos_layout && columns.len() == 6 {
            columns.push("");
        }
        if macos_layout {
            assert_eq!(
                columns.len(),
                7,
                "macOS fixture row must have 7 columns: {line}"
            );
            if columns[2].is_empty() {
                continue;
            }
        }
        let (bytes_hex, code, modifiers) = fixture_fields(&columns);
        if !macos_layout {
            assert!(
                bytes_hex.chars().all(|ch| ch.is_ascii_hexdigit()),
                "non-hex fixture bytes: {bytes_hex} in {line}"
            );
        }
        let bytes = decode_hex(bytes_hex);
        let (event, consumed) = extract_one_event(&bytes).unwrap();
        assert_eq!(
            consumed,
            bytes.len(),
            "fixture should extract a whole event: {line}"
        );
        assert_raw_key(
            event,
            &parse_fixture_key_code(code),
            parse_fixture_modifiers(modifiers),
        );
    }
}

/// Select the existing keyboard or terminal fixture column layout.
fn fixture_fields<'a>(columns: &[&'a str]) -> (&'a str, &'a str, &'a str) {
    match columns.len() {
        6 if columns[1].chars().all(|ch| ch.is_ascii_hexdigit()) => {
            (columns[1], columns[2], columns[3])
        }
        6 | 7 => (columns[2], columns[3], columns[4]),
        _ => panic!("fixture row must have 6 or 7 columns: {columns:?}"),
    }
}

/// Test host numeric admission independently of the geometry consumer.
fn assert_host_mouse_admission() {
    for (data, expected) in [
        (b"\x1b[<0;1;2M".as_slice(), Some((0, 1, false))),
        (b"\x1b[<0;1;2m", Some((0, 1, true))),
        (b"\x1b[<0;0;0M", None),
        (b"\x1b[<256;65536;4294967295M", None),
        (b"\x1b[<0;65535;65535M", Some((65534, 65534, false))),
        (b"\x1b[<0;65536;1M", None),
        (b"\x1b[<0;1;2;9M", Some((0, 1, false))),
        (b"\x1b[<0;1;2;M", Some((0, 1, false))),
        (b"\x1b[<+0;+1;+2M", Some((0, 1, false))),
        (b"\x1b[<0;;2M", None),
        (b"\x1b[<0;4294967296;2M", None),
        (b"\x1b[<255;1;2M", None),
    ] {
        let events = parse_raw_input_bytes_sync(data);
        assert_eq!(events.len(), 1);
        match expected {
            Some((x, y, release)) => {
                let RawInputEvent::Mouse {
                    kind,
                    column,
                    row,
                    modifiers,
                } = events[0]
                else {
                    panic!("{data:?}");
                };
                assert_eq!((column, row, modifiers), (x, y, 0));
                assert_eq!(
                    kind,
                    if release {
                        MouseEventKind::Up(MouseButton::Left)
                    } else {
                        MouseEventKind::Down(MouseButton::Left)
                    }
                );
            }
            None => assert!(matches!(events[0], RawInputEvent::Unsupported), "{data:?}"),
        }
    }
}

/// Exercise every admitted button class and modifier combination through projection.
fn assert_host_mouse_classes() {
    let classes = [
        (0, MouseEventKind::Down(MouseButton::Left)),
        (1, MouseEventKind::Down(MouseButton::Middle)),
        (2, MouseEventKind::Down(MouseButton::Right)),
        (3, MouseEventKind::Up(MouseButton::Left)),
        (32, MouseEventKind::Drag(MouseButton::Left)),
        (33, MouseEventKind::Drag(MouseButton::Middle)),
        (34, MouseEventKind::Drag(MouseButton::Right)),
        (35, MouseEventKind::Moved),
        (64, MouseEventKind::ScrollUp),
        (65, MouseEventKind::ScrollDown),
        (66, MouseEventKind::ScrollLeft),
        (67, MouseEventKind::ScrollRight),
        (96, MouseEventKind::Moved),
        (97, MouseEventKind::Moved),
        (160, MouseEventKind::Moved),
        (161, MouseEventKind::Moved),
    ];
    for (cb, expected_kind) in classes {
        for (bits, wire) in [
            (0, 0),
            (4, 1),
            (8, 4),
            (16, 2),
            (12, 5),
            (20, 3),
            (24, 6),
            (28, 7),
        ] {
            for terminator in ['M', 'm'] {
                assert_host_mouse_class(cb | bits, wire, terminator, expected_kind);
            }
        }
    }
}

#[test]
fn structured_key_projection_keeps_source_and_text() {
    use maestro_protocol::frames::{ClientInputEvent, ClientKeySource};
    for (phase, kind, expected_text) in [
        (1, KeyEventKind::Press, Some("🙂語")),
        (2, KeyEventKind::Repeat, Some("🙂語")),
        (3, KeyEventKind::Release, None),
    ] {
        let report = format!("\x1b[108:76;2:{phase};128578:35486u");
        let direct = crate::input::parse_terminal_key_sequence(&report).unwrap();
        assert_eq!(direct.shifted_codepoint, Some(u32::from('L')));
        assert_eq!(direct.vt_bytes(), None);
        let mut framer = RawInputFramer::default();
        assert!(framer.push(&report.as_bytes()[..5]).is_empty());
        let mut events = framer.push(&report.as_bytes()[5..]);
        assert_eq!(events.len(), 1);
        let RawInputEvent::Key(key) = events.remove(0) else {
            panic!("expected key");
        };
        assert_eq!(key.shifted_codepoint, Some(u32::from('L')));
        assert_eq!(key.repeat_count, 1);
        assert_eq!(key.generated_text.as_deref(), expected_text);
        let wire: ClientInputEvent = key.into();
        assert_eq!(
            wire,
            ClientInputEvent::Key {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::SHIFT,
                kind,
                repeat_count: 1,
                generated_text: expected_text.map(str::to_owned),
                source: ClientKeySource::Vt {
                    bytes: report.into_bytes()
                },
            }
        );
    }
    let direct = crate::input::parse_terminal_key_sequence("\x1b[108:76;2u").unwrap();
    let wire: ClientInputEvent = direct.into();
    assert!(matches!(
        wire,
        ClientInputEvent::Key {
            source: ClientKeySource::Synthesized,
            ..
        }
    ));
}

/// Check one admitted mouse class with an original report and exact byte count.
fn assert_host_mouse_class(cb: u8, wire: u8, terminator: char, expected_kind: MouseEventKind) {
    let report = format!("\x1b[<{cb};20;10{terminator}");
    let (
        RawInputEvent::Mouse {
            kind,
            column,
            row,
            modifiers,
        },
        consumed,
    ) = extract_one_event(report.as_bytes()).unwrap()
    else {
        panic!("{report:?}");
    };
    let expected = match (expected_kind, terminator) {
        (MouseEventKind::Down(button), 'm') => MouseEventKind::Up(button),
        (kind, _) => kind,
    };
    assert_eq!(
        (kind, column, row, modifiers, consumed),
        (expected, 19, 9, wire, report.len())
    );
}

/// Verify each event range selects its original input bytes.
fn assert_source_ranges(input: &[u8], ranges: &[RangedEvent]) {
    assert_eq!(
        &input[ranges[0].start..ranges[0].start + ranges[0].len],
        b"\x1b[A"
    );
    assert_eq!(
        &input[ranges[1].start..ranges[1].start + ranges[1].len],
        b"a"
    );
    assert_eq!(
        &input[ranges[2].start..ranges[2].start + ranges[2].len],
        b"\x1b[B"
    );
}

/// Assert a range's projected key identity.
fn assert_ranged_key(range: &RangedEvent, expected: &KeyCode) {
    let RawInputEvent::Key(key) = &range.event else {
        panic!("expected key");
    };
    assert_eq!(&key.code, expected);
}

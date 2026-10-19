//! Structured host keyboard reports.

use maestro_tui::keys::{
    Code, KeyEventType, Named, Sequence, is_key_release, parse_key, parse_terminal_key_sequence,
};

#[test]
fn host_parse_extended_sequence_preserves_shifted_symbol_pair() {
    let key = parse_terminal_key_sequence("\x1b[49:33;2:1u").unwrap();
    assert_eq!(key.code, Code::Char('1'));
    assert_eq!(key.modifier, 1);
    assert_eq!(key.event_type, KeyEventType::Press);
    assert_eq!(key.shifted, Some(u32::from('!')));
}

#[test]
fn host_parse_extended_sequence_recovers_omitted_shift_modifier() {
    for (sequence, phase) in [
        ("\x1b[114:82;1u", KeyEventType::Press),
        ("\x1b[114:82;1:2u", KeyEventType::Repeat),
        ("\x1b[114:82;1:3u", KeyEventType::Release),
    ] {
        let key = parse_terminal_key_sequence(sequence).unwrap();
        assert_eq!(key.code, Code::Char('r'));
        assert_eq!(key.modifier, 1);
        assert_eq!(key.event_type, phase);
        assert_eq!(key.shifted, Some(u32::from('R')));
    }
}

#[test]
fn host_parse_extended_sequence_with_associated_emoji_text() {
    let key = parse_terminal_key_sequence("\x1b[128512;1;128512u").unwrap();
    assert_eq!(key.code, Code::Char('😀'));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Press);
    assert_eq!(key.shifted, None);
    assert_eq!(key.generated_text.as_deref(), Some("😀"));
}

#[test]
fn host_parse_extended_sequence_preserves_shifted_letter_pair_and_release() {
    let key = parse_terminal_key_sequence("\x1b[108:76;2:3u").unwrap();
    assert_eq!(key.code, Code::Char('l'));
    assert_eq!(key.modifier, 1);
    assert_eq!(key.event_type, KeyEventType::Release);
    assert_eq!(key.shifted, Some(u32::from('L')));
}

#[test]
fn host_parse_extended_sequence_does_not_infer_shift_without_distinct_shifted_alternate() {
    for sequence in ["\x1b[114;1u", "\x1b[114:114;1u", "\x1b[114::113;1u"] {
        let key = parse_terminal_key_sequence(sequence).unwrap();
        assert_eq!(key.code, Code::Char('r'));
        assert_eq!(key.modifier, 0);
    }
}

#[test]
fn host_parse_extended_sequence_preserves_non_us_shift_pairs() {
    for (sequence, base, shifted) in [
        ("\x1b[50:34;2:1u", '2', '"'),
        ("\x1b[38:49;2:1u", '&', '1'),
        ("\x1b[305:73;2:1u", 'ı', 'I'),
        ("\x1b[287:286;2:1u", 'ğ', 'Ğ'),
    ] {
        let key = parse_terminal_key_sequence(sequence).unwrap();
        assert_eq!(key.code, Code::Char(base));
        assert_eq!(key.modifier, 1);
        assert_eq!(key.event_type, KeyEventType::Press);
        assert_eq!(key.shifted, Some(u32::from(shifted)));
    }
}

#[test]
fn host_parse_extended_sequence_with_multicodepoint_ime_text() {
    let key = parse_terminal_key_sequence("\x1b[32;;20320:22909u").unwrap();
    assert_eq!(key.code, Code::Char(' '));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Press);
    assert_eq!(key.generated_text.as_deref(), Some("你好"));
}

#[test]
fn host_reject_malformed_extended_associated_text() {
    for sequence in [
        "\x1b[32;;1114112u",
        "\x1b[32;;20320:bad:u",
        "\x1b[32;;27u",
        "\x1b[32;;133u",
    ] {
        assert_eq!(parse_terminal_key_sequence(sequence), None);
    }
}

#[test]
fn strict_host_events_do_not_change_chat_event_admission() {
    let _globals = crate::input_support::globals();
    let expected = parse_terminal_key_sequence("\x1b[97;1;97u");
    for active in [false, true] {
        maestro_tui::set_kitty_protocol_active(active);
        assert_eq!(parse_terminal_key_sequence("\x1b[97;1;97u"), expected);
    }
    maestro_tui::set_kitty_protocol_active(false);

    for event in ["0", "4", "01", "4294967296"] {
        let sequence = format!("\x1b[97;1:{event}u");
        assert_eq!(parse_terminal_key_sequence(&sequence), None);
        assert_eq!(parse_key(&sequence).as_deref(), Some("a"));
    }
    assert_eq!(
        parse_terminal_key_sequence("\x1b[97;1:3u")
            .unwrap()
            .event_type,
        KeyEventType::Release
    );
    assert!(is_key_release("\x1b[97;1:3u"));
    assert!(parse_terminal_key_sequence("\x1b[97;1;97u").is_some());
    assert_eq!(parse_key("\x1b[97;1;97u"), None);
}

#[test]
fn structured_numeric_and_associated_text_boundaries() {
    assert_scalar_and_default_modifiers();
    let keypad = parse_terminal_key_sequence("\x1b[57399:48;1u").unwrap();
    assert_eq!(keypad.code, Code::Char('0'));
    assert_eq!(keypad.modifier, 1);
    assert_eq!(keypad.shifted, Some(48));
    let backtab = parse_terminal_key_sequence("\x1b[Z").unwrap();
    assert_eq!(backtab.code, Code::Key(Named::BackTab));
    assert_eq!(backtab.modifier, 1);
    for (sequence, shifted) in [
        ("\x1b[97:55296;1u", Some(55296)),
        ("\x1b[97:1114112;1u", Some(1_114_112)),
        ("\x1b[97:4294967295;1u", Some(u32::MAX)),
        ("\x1b[97:4294967296;1u", None),
        ("\x1b[97:bad;1u", None),
        ("\x1b[97::bad:ignored;1u", None),
        ("\x1b[+97:+97;+1u", Some(97)),
    ] {
        let key = parse_terminal_key_sequence(sequence).unwrap();
        assert_eq!(key.code, Code::Char('a'));
        assert_eq!(key.shifted, shifted);
        assert_eq!(key.modifier, 0);
    }
    assert_eq!(
        parse_terminal_key_sequence("\x1b[97;255u")
            .unwrap()
            .modifier,
        254
    );
    for sequence in [
        "\x1b[97;0u",
        "\x1b[97;256u",
        "\x1b[4294967296;1u",
        "\x1b[55296;1u",
        "\x1b[1114112;1u",
        "\x1b[97;1:01u",
        "\x1b[97;1;u",
        "\x1b[97;1;97;98u",
        "\x1b[97;1;55296u",
    ] {
        assert_eq!(parse_terminal_key_sequence(sequence), None);
    }
    assert_eq!(
        parse_terminal_key_sequence("\x1b[97;1;+97u")
            .unwrap()
            .generated_text
            .as_deref(),
        Some("a")
    );
}

#[test]
fn host_parse_legacy_f_keys() {
    for (data, named) in [
        ("\x1bOP", Named::Function(1)),
        ("\x1b[11~", Named::Function(1)),
        ("\x1bOQ", Named::Function(2)),
        ("\x1b[12~", Named::Function(2)),
        ("\x1bOR", Named::Function(3)),
        ("\x1b[13~", Named::Function(3)),
        ("\x1bOS", Named::Function(4)),
        ("\x1b[14~", Named::Function(4)),
        ("\x1b[15~", Named::Function(5)),
        ("\x1b[1~", Named::Home),
        ("\x1b[4~", Named::End),
        ("\x1b[5~", Named::PageUp),
        ("\x1b[6~", Named::PageDown),
    ] {
        let key = parse_terminal_key_sequence(data).unwrap();
        assert_eq!(key.code, Code::Key(named));
        assert_eq!(key.modifier, 0);
        assert_eq!(key.event_type, KeyEventType::Press);
        assert_eq!(key.shifted, None);
    }
    for data in ["\x1b[10~", "\x1b[16~"] {
        assert_eq!(parse_terminal_key_sequence(data), None);
    }
}

/// Checks all structured fields retained by the host decoder.
fn assert_terminal_key_eq(
    actual: &Sequence,
    code: Code,
    modifier: u32,
    event_type: KeyEventType,
    shifted: Option<u32>,
) {
    assert_eq!(actual.code, code);
    assert_eq!(actual.modifier, modifier);
    assert_eq!(actual.event_type, event_type);
    assert_eq!(actual.shifted, shifted);
}

#[test]
fn host_parse_legacy_application_keypad_sequences() {
    let cases = [
        ("\x1bOp", Code::Char('0')),
        ("\x1bOq", Code::Char('1')),
        ("\x1bOr", Code::Char('2')),
        ("\x1bOs", Code::Char('3')),
        ("\x1bOt", Code::Char('4')),
        ("\x1bOu", Code::Char('5')),
        ("\x1bOv", Code::Char('6')),
        ("\x1bOw", Code::Char('7')),
        ("\x1bOx", Code::Char('8')),
        ("\x1bOy", Code::Char('9')),
        ("\x1bOn", Code::Char('.')),
        ("\x1bOl", Code::Char(',')),
        ("\x1bOm", Code::Char('-')),
        ("\x1bOk", Code::Char('+')),
        ("\x1bOj", Code::Char('*')),
        ("\x1bOo", Code::Char('/')),
        ("\x1bOM", Code::Key(Named::Enter)),
    ];

    for (sequence, code) in cases {
        assert_terminal_key_eq(
            &parse_terminal_key_sequence(sequence).expect("keypad sequence should parse"),
            code,
            0,
            KeyEventType::Press,
            None,
        );
    }
}

#[test]
fn host_parse_legacy_alt_shift_letter_preserves_shift() {
    let key = parse_terminal_key_sequence("\x1bA").expect("alt-shift letter should parse");
    assert_terminal_key_eq(&key, Code::Char('A'), 2 | 1, KeyEventType::Press, None);
}

#[test]
fn host_parse_legacy_alt_control_letter_composes_modifiers() {
    let key =
        parse_terminal_key_sequence("\x1b\x06").expect("ctrl-alt-f legacy sequence should parse");
    assert_terminal_key_eq(&key, Code::Char('f'), 4 | 2, KeyEventType::Press, None);
}

#[test]
fn host_unknown_legacy_ss3_sequence_remains_unsupported() {
    assert!(parse_terminal_key_sequence("\x1bOz").is_none());
}

#[test]
fn host_parse_modified_f_keys() {
    assert_terminal_key_eq(
        &parse_terminal_key_sequence("\x1b[1;2P").expect("shift+f1 should parse"),
        Code::Key(Named::Function(1)),
        1,
        KeyEventType::Press,
        None,
    );
    assert_terminal_key_eq(
        &parse_terminal_key_sequence("\x1b[1;3S").expect("alt+f4 should parse"),
        Code::Key(Named::Function(4)),
        2,
        KeyEventType::Press,
        None,
    );
    assert_terminal_key_eq(
        &parse_terminal_key_sequence("\x1b[1;4S").expect("shift+alt+f4 should parse"),
        Code::Key(Named::Function(4)),
        1 | 2,
        KeyEventType::Press,
        None,
    );
    assert_terminal_key_eq(
        &parse_terminal_key_sequence("\x1b[15;2~").expect("shift+f5 should parse"),
        Code::Key(Named::Function(5)),
        1,
        KeyEventType::Press,
        None,
    );
    assert_eq!(parse_terminal_key_sequence("\x1b[11;2~"), None);
    assert_eq!(parse_terminal_key_sequence("\x1b[14;1~"), None);
    assert_eq!(parse_terminal_key_sequence("\x1b[14;3~"), None);
}

#[test]
fn host_parse_extended_alt_backspace_sequence() {
    let key = parse_terminal_key_sequence("\x1b[127;3u").unwrap();
    assert_eq!(key.code, Code::Key(Named::Backspace));
    assert_eq!(key.modifier, 2);
    assert_eq!(key.event_type, KeyEventType::Press);
    assert_eq!(key.shifted, None);
}

#[test]
fn host_parse_modify_other_keys_sequence() {
    let key = parse_terminal_key_sequence("\x1b[27;6;108~").unwrap();
    assert_eq!(key.code, Code::Char('l'));
    assert_eq!(key.modifier, 4 | 1);
    assert_eq!(key.event_type, KeyEventType::Press);
    assert_eq!(key.shifted, None);
}

#[test]
fn host_parse_legacy_uppercase_letter_as_shifted_char() {
    let key = parse_terminal_key_sequence("L").unwrap();
    assert_eq!(key.code, Code::Char('L'));
    assert_eq!(key.modifier, 1);
}

#[test]
fn host_parse_legacy_up_arrow_sequence() {
    let key = parse_terminal_key_sequence("\x1b[A").unwrap();
    assert_eq!(key.code, Code::Key(Named::Up));
    assert_eq!(key.modifier, 0);
}

#[test]
fn host_parse_legacy_alt_backspace_sequence() {
    let key = parse_terminal_key_sequence("\x1b\x7f").unwrap();
    assert_eq!(key.code, Code::Key(Named::Backspace));
    assert_eq!(key.modifier, 2);
}

#[test]
fn host_parse_extended_modifier_sequence() {
    let key = parse_terminal_key_sequence("\x1b[57441;2:1u").unwrap();
    assert_eq!(key.code, Code::Key(Named::LeftShift));
    assert_eq!(key.modifier, 1);
    assert_eq!(key.event_type, KeyEventType::Press);
}

#[test]
fn host_parse_enhanced_up_arrow_press_sequence() {
    let key = parse_terminal_key_sequence("\x1b[1;1:1A").unwrap();
    assert_eq!(key.code, Code::Key(Named::Up));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Press);
}

#[test]
fn host_parse_enhanced_up_arrow_release_sequence() {
    let key = parse_terminal_key_sequence("\x1b[1;1:3A").unwrap();
    assert_eq!(key.code, Code::Key(Named::Up));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Release);
}

#[test]
fn host_parse_enhanced_pageup_press_sequence() {
    let key = parse_terminal_key_sequence("\x1b[5;1:1~").unwrap();
    assert_eq!(key.code, Code::Key(Named::PageUp));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Press);
}

#[test]
fn host_parse_enhanced_pagedown_release_sequence() {
    let key = parse_terminal_key_sequence("\x1b[6;1:3~").unwrap();
    assert_eq!(key.code, Code::Key(Named::PageDown));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Release);
}

#[test]
fn host_parse_enhanced_delete_repeat_sequence() {
    let key = parse_terminal_key_sequence("\x1b[3;1:2~").unwrap();
    assert_eq!(key.code, Code::Key(Named::Delete));
    assert_eq!(key.modifier, 0);
    assert_eq!(key.event_type, KeyEventType::Repeat);
}

#[test]
fn host_parse_xterm_alt_up_arrow_sequence() {
    let key = parse_terminal_key_sequence("\x1b[1;3A").unwrap();
    assert_eq!(key.code, Code::Key(Named::Up));
    assert_eq!(key.modifier, 2);
}

#[test]
fn host_parse_xterm_alt_down_arrow_sequence() {
    let key = parse_terminal_key_sequence("\x1b[1;3B").unwrap();
    assert_eq!(key.code, Code::Key(Named::Down));
    assert_eq!(key.modifier, 2);
}

#[test]
fn host_parse_extended_functional_up_arrow_sequence() {
    let key = parse_terminal_key_sequence("\x1b[57419;1u").unwrap();
    assert_eq!(key.code, Code::Key(Named::Up));
    assert_eq!(key.modifier, 0);
}

#[test]
fn host_parse_legacy_ctrl_b_sequence() {
    let key = parse_terminal_key_sequence("\x02").unwrap();
    assert_eq!(key.code, Code::Char('b'));
    assert_eq!(key.modifier, 4);
}

#[test]
fn host_parse_legacy_ctrl_c_sequence() {
    let key = parse_terminal_key_sequence("\x03").unwrap();
    assert_eq!(key.code, Code::Char('c'));
    assert_eq!(key.modifier, 4);
}

#[test]
fn host_parse_legacy_lf_sequence_as_ctrl_j() {
    let key = parse_terminal_key_sequence("\n").unwrap();
    assert_eq!(key.code, Code::Char('j'));
    assert_eq!(key.modifier, 4);
}

#[test]
fn host_legacy_ctrl_byte_matrix_is_covered() {
    for (byte, expected) in [
        (b'\x01', 'a'),
        (b'\x02', 'b'),
        (b'\x03', 'c'),
        (b'\x1a', 'z'),
    ] {
        let key = parse_terminal_key_sequence(std::str::from_utf8(&[byte]).unwrap()).unwrap();
        assert_terminal_key_eq(&key, Code::Char(expected), 4, KeyEventType::Press, None);
    }

    for (byte, expected) in [
        (b'\x1c', '\\'),
        (b'\x1d', ']'),
        (b'\x1e', '^'),
        (b'\x1f', '_'),
    ] {
        let key = parse_terminal_key_sequence(std::str::from_utf8(&[byte]).unwrap()).unwrap();
        assert_terminal_key_eq(&key, Code::Char(expected), 4, KeyEventType::Press, None);
    }
}

#[test]
fn host_extended_functional_key_matrix_is_covered() {
    for (point, key) in native_functional_cases() {
        assert_terminal_key_eq(
            &parse_terminal_key_sequence(&format!("\x1b[{point};1u")).unwrap(),
            Code::Key(key),
            0,
            KeyEventType::Press,
            None,
        );
    }
    for (point, number) in (57376..=57398).zip([
        13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
    ]) {
        assert_terminal_key_eq(
            &parse_terminal_key_sequence(&format!("\x1b[{point};1u")).unwrap(),
            Code::Key(Named::Function(number)),
            0,
            KeyEventType::Press,
            None,
        );
    }

    let cases = [
        ("\x1b[57399;1u", Code::Char('0')),
        ("\x1b[57400;1u", Code::Char('1')),
        ("\x1b[57401;1u", Code::Char('2')),
        ("\x1b[57402;1u", Code::Char('3')),
        ("\x1b[57403;1u", Code::Char('4')),
        ("\x1b[57404;1u", Code::Char('5')),
        ("\x1b[57405;1u", Code::Char('6')),
        ("\x1b[57406;1u", Code::Char('7')),
        ("\x1b[57407;1u", Code::Char('8')),
        ("\x1b[57408;1u", Code::Char('9')),
        ("\x1b[57409;1u", Code::Char('.')),
        ("\x1b[57410;1u", Code::Char('/')),
        ("\x1b[57411;1u", Code::Char('*')),
        ("\x1b[57412;1u", Code::Char('-')),
        ("\x1b[57413;1u", Code::Char('+')),
        ("\x1b[57414;1u", Code::Key(Named::Enter)),
        ("\x1b[57415;1u", Code::Char('=')),
        ("\x1b[57416;1u", Code::Char(',')),
        ("\x1b[57417;1u", Code::Key(Named::Left)),
        ("\x1b[57418;1u", Code::Key(Named::Right)),
        ("\x1b[57419;1u", Code::Key(Named::Up)),
        ("\x1b[57420;1u", Code::Key(Named::Down)),
        ("\x1b[57421;1u", Code::Key(Named::PageUp)),
        ("\x1b[57422;1u", Code::Key(Named::PageDown)),
        ("\x1b[57423;1u", Code::Key(Named::Home)),
        ("\x1b[57424;1u", Code::Key(Named::End)),
        ("\x1b[57425;1u", Code::Key(Named::Insert)),
        ("\x1b[57426;1u", Code::Key(Named::Delete)),
    ];

    for (sequence, code) in cases {
        let parsed = parse_terminal_key_sequence(sequence).unwrap();
        assert_terminal_key_eq(&parsed, code, 0, KeyEventType::Press, None);
    }
}

#[test]
fn host_unknown_extended_functional_key_remains_unsupported() {
    assert!(parse_terminal_key_sequence("\x1b[57364;1u").is_none());
}

/// Native functional identities retained before daemon projection.
fn native_functional_cases() -> [(u32, Named); 34] {
    [
        (57358, Named::CapsLock),
        (57359, Named::ScrollLock),
        (57360, Named::NumLock),
        (57361, Named::PrintScreen),
        (57362, Named::Pause),
        (57363, Named::Menu),
        (57427, Named::KeypadBegin),
        (57428, Named::MediaPlay),
        (57429, Named::MediaPause),
        (57430, Named::MediaPlayPause),
        (57431, Named::MediaReverse),
        (57432, Named::MediaStop),
        (57433, Named::MediaFastForward),
        (57434, Named::MediaRewind),
        (57435, Named::MediaTrackNext),
        (57436, Named::MediaTrackPrevious),
        (57437, Named::MediaRecord),
        (57438, Named::MediaLowerVolume),
        (57439, Named::MediaRaiseVolume),
        (57440, Named::MediaMuteVolume),
        (57441, Named::LeftShift),
        (57442, Named::LeftControl),
        (57443, Named::LeftAlt),
        (57444, Named::LeftSuper),
        (57445, Named::LeftHyper),
        (57446, Named::LeftMeta),
        (57447, Named::RightShift),
        (57448, Named::RightControl),
        (57449, Named::RightAlt),
        (57450, Named::RightSuper),
        (57451, Named::RightHyper),
        (57452, Named::RightMeta),
        (57453, Named::IsoLevel3Shift),
        (57454, Named::IsoLevel5Shift),
    ]
}

/// Reads the fixed expected key vocabulary used by the recorded terminal samples.
fn fixture_code(value: &str) -> Code {
    let key = match value {
        "enter" => Named::Enter,
        "tab" => Named::Tab,
        "backspace" => Named::Backspace,
        "esc" => Named::Escape,
        "up" => Named::Up,
        "down" => Named::Down,
        "left" => Named::Left,
        "right" => Named::Right,
        "home" => Named::Home,
        "end" => Named::End,
        "pageup" => Named::PageUp,
        "pagedown" => Named::PageDown,
        "insert" => Named::Insert,
        "delete" => Named::Delete,
        value => return Code::Char(value.strip_prefix("char:").unwrap().chars().next().unwrap()),
    };
    Code::Key(key)
}

/// Checks recorded bytes against their independently recorded structured fields.
fn assert_fixture_corpus_parses(corpus: &str) {
    for line in corpus
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let columns: Vec<_> = line.split('\t').collect();
        let start = if columns[1].chars().all(|ch| ch.is_ascii_hexdigit()) {
            1
        } else {
            2
        };
        let columns = &columns[start..];
        let bytes: Vec<_> = columns[0]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let parsed = parse_terminal_key_sequence(std::str::from_utf8(&bytes).unwrap())
            .unwrap_or_else(|| panic!("fixture failed: {line}"));
        let modifier = columns[2].split('+').fold(0, |bits, label| {
            bits | match label {
                "-" | "" => 0,
                "shift" => 1,
                "alt" => 2,
                "control" => 4,
                "super" => 8,
                "hyper" => 16,
                "meta" => 32,
                other => panic!("unknown modifier: {other}"),
            }
        });
        let phase = match columns[3] {
            "press" => KeyEventType::Press,
            "repeat" => KeyEventType::Repeat,
            "release" => KeyEventType::Release,
            other => panic!("unknown phase: {other}"),
        };
        let shifted = columns
            .get(4)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse().unwrap());
        assert_terminal_key_eq(&parsed, fixture_code(columns[1]), modifier, phase, shifted);
    }
}

#[test]
fn host_keyboard_protocol_corpus_fixture_parses() {
    assert_fixture_corpus_parses(include_str!("fixtures/host_input/keyboard_reports.tsv"));
}

#[test]
fn host_macos_terminal_variants_fixture_parses() {
    assert_fixture_corpus_parses(include_str!("fixtures/host_input/macos_key_samples.tsv"));
}

#[test]
fn host_linux_terminal_variants_fixture_parses() {
    assert_fixture_corpus_parses(include_str!("fixtures/host_input/linux_key_samples.tsv"));
}

/// Checks scalar admission and omitted versus explicit default modifiers.
fn assert_scalar_and_default_modifiers() {
    for (data, code, modifier) in [
        ("\x1b[97u", Code::Char('a'), 0),
        ("\x1b[97;1u", Code::Char('a'), 0),
        ("\x1b[1114111;1u", Code::Char('\u{10ffff}'), 0),
        ("\x1b[97;1;97u", Code::Char('a'), 0),
    ] {
        let key = parse_terminal_key_sequence(data).unwrap();
        assert_eq!(key.code, code);
        assert_eq!(key.modifier, modifier);
        assert_eq!(key.event_type, KeyEventType::Press);
    }
}

#[test]
fn shared_byte_framing_keeps_chat_and_host_policies_independent() {
    assert_chat_framing_policy();
    assert_host_framing_policy();
    assert_host_mouse_framing();
}

/// Historical chat outputs for mixed text, paste, mouse, byte and duplicate input.
fn assert_chat_framing_policy() {
    use maestro_tui::{
        StdinBuffer, StdinBufferEventMap as Event, StdinBufferInput, StdinBufferOptions,
    };
    use std::time::{Duration, Instant};

    let now = Instant::now();
    let mut chat = StdinBuffer::new(StdinBufferOptions::default());
    assert_eq!(
        chat.process(StdinBufferInput::Text(""), now),
        [Event::Data(String::new())]
    );
    assert_eq!(
        chat.process(
            StdinBufferInput::Text("é\x1b[200~你好\x1b[201~\x1b[M é界"),
            now
        ),
        [
            Event::Data("é".into()),
            Event::Paste("你好".into()),
            Event::Data("\x1b[M é界".into())
        ]
    );
    assert_eq!(
        chat.process(StdinBufferInput::Bytes(&[0xe9]), now),
        [Event::Data("\x1bi".into())]
    );
    assert_eq!(
        chat.process(StdinBufferInput::Text("\x1b[97ua"), now),
        [Event::Data("\x1b[97u".into())]
    );
    assert!(
        chat.process(StdinBufferInput::Text("\x1b["), now)
            .is_empty()
    );
    assert_eq!(chat.deadline(), now.checked_add(Duration::from_millis(10)));
    assert_eq!(
        chat.expire(now + Duration::from_millis(10)),
        [Event::Data("\x1b[".into())]
    );
}

/// Host bytes retain split UTF-8 and binary mouse without chat duplicate filtering.
fn assert_host_framing_policy() {
    use maestro_tui::stdin_buffer::RawInputByteFramer;
    let mut host = RawInputByteFramer::default();
    assert!(host.push(&[]).is_empty());
    assert_eq!(
        host.push(b"\x1b[97ua"),
        [b"\x1b[97u".to_vec(), b"a".to_vec()]
    );
    assert!(host.push(&[0xc3]).is_empty());
    assert!(!host.has_pending_incomplete_mouse_sequence());
    assert!(host.flush_timeout().is_empty());
    assert_eq!(host.push(&[0xa9]), ["é".as_bytes()]);
    assert!(host.push(b"\x1b[M").is_empty());
    assert!(host.has_pending_incomplete_mouse_sequence());
    assert_eq!(host.push(&[32, 0xff, 33]), [vec![27, 91, 77, 32, 255, 33]]);
    assert!(!host.has_pending_incomplete_mouse_sequence());
    assert!(host.push(b"\x1b[200~paste").is_empty());
    assert!(host.has_pending_bracketed_paste());
    assert!(host.flush_timeout().is_empty());
    assert_eq!(
        host.push(b"\x1b[201~"),
        [b"\x1b[200~paste\x1b[201~".to_vec()]
    );
}

/// Mouse validity is shared by doubled-Escape splitting and orphaned-tail disposal.
fn assert_host_mouse_framing() {
    use maestro_tui::stdin_buffer::RawInputByteFramer;
    for report in [
        b"\x1b[<35;10;20M".as_slice(),
        b"\x1b[<35;10;20m",
        b"\x1b[MCN1",
    ] {
        let mut host = RawInputByteFramer::default();
        assert!(host.push(b"\x1b").is_empty());
        assert_eq!(host.push(report), [b"\x1b".to_vec(), report.to_vec()]);
        assert!(host.flush_timeout().is_empty());
    }
    let mut host = RawInputByteFramer::default();
    assert!(host.push(b"\x1b").is_empty());
    assert_eq!(host.flush_timeout(), [b"\x1b".to_vec()]);
    assert_eq!(host.push(b"[<35;10;20Mx"), [b"x".to_vec()]);
}

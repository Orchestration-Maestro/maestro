//! Standalone operating-system command decoding.

use maestro_terminal::osc::{Command, Parser};

#[test]
fn decode_osc_0_change_window_title() {
    let mut parser = Parser::init(false);
    for byte in b"0;title" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(Some(7)),
        Some(Command::ChangeWindowTitle(b"title".to_vec()))
    );
}

#[test]
fn decode_osc_52_get_set_clipboard_optional_parameter() {
    let mut parser = Parser::init(false);
    for byte in b"52;;?" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(None),
        Some(Command::ClipboardContents {
            kind: b'c',
            data: b"?".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_id_set() {
    let mut parser = Parser::init(false);
    for byte in b"8;id=one;uri" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(None),
        Some(Command::HyperlinkStart {
            id: Some(b"one".to_vec()),
            uri: b"uri".to_vec()
        })
    );
}

/// Feed one complete standalone command.
fn decode(input: &[u8], alloc: bool, term: Option<u8>) -> Option<Command> {
    let mut parser = Parser::init(alloc);
    for byte in input {
        parser.next(*byte);
    }
    parser.end(term)
}

#[test]
fn decode_osc_133_prompt_start_with_k_r() {
    use maestro_terminal::osc::semantic_prompt::{
        Action, Option as PromptOption, OptionValue, PromptKind,
    };
    let Some(Command::SemanticPrompt(command)) = decode(b"133;P;k=r", false, None) else {
        panic!("missing prompt");
    };
    assert_eq!(command.action, Action::PromptStart);
    assert_eq!(command.options_unvalidated, b"k=r");
    assert_eq!(
        command.read_option(PromptOption::PromptKind),
        Some(OptionValue::PromptKind(PromptKind::Right))
    );
}

#[test]
fn osc_special_reset_prefix_and_list_are_decoded() {
    for input in [b"105".as_slice(), b"105;", b"105;bad", b"105;0;4;5"] {
        let requests = if input == b"105;0;4;5" {
            vec![
                Request::Reset(Target::Special(Special::Bold)),
                Request::Reset(Target::Special(Special::Italic)),
            ]
        } else {
            vec![Request::ResetSpecial]
        };
        for alloc in [false, true] {
            for term in [None, Some(7), Some(27)] {
                let expected = alloc.then_some(Command::ColorOperation {
                    op: Operation::Osc105,
                    requests: requests.clone(),
                    terminator: Terminator::init(term),
                });
                assert_eq!(decode(input, alloc, term), expected);
            }
        }
    }
    assert_color(
        b"104;0;4;5",
        Operation::Osc104,
        vec![
            Request::Reset(Target::Palette(0)),
            Request::Reset(Target::Palette(4)),
            Request::Reset(Target::Palette(5)),
        ],
    );
    assert_color(b"112;0", Operation::Osc112, vec![]);
}

#[test]
fn decode_osc_21_kitty_color_protocol_double_reset() {
    use maestro_terminal::{
        kitty::color::{Kind, Request, Special},
        osc::Terminator,
    };
    let Some(Command::KittyColorProtocol(command)) = decode(b"21;cursor=;cursor=", true, Some(7))
    else {
        panic!("missing colour protocol command");
    };
    assert_eq!(
        command.list,
        vec![
            Request::Reset(Kind::Special(Special::Cursor)),
            Request::Reset(Kind::Special(Special::Cursor))
        ]
    );
    assert_eq!(command.terminator, Terminator::Bel);
}

#[test]
fn decode_osc_1337_test_currentdir_with_non_empty_value() {
    assert_eq!(
        decode(b"1337;CurrentDir=/tmp", false, None),
        Some(Command::ReportPwd {
            value: b"/tmp".to_vec()
        })
    );
}

#[test]
fn decode_osc_7_report_pwd() {
    assert_eq!(
        decode(b"7;file:///tmp", false, None),
        Some(Command::ReportPwd {
            value: b"file:///tmp".to_vec()
        })
    );
}

#[test]
fn decode_osc_9_9_conemu_set_current_working_directory() {
    assert_eq!(
        decode(b"9;9;/tmp", false, None),
        Some(Command::ReportPwd {
            value: b"/tmp".to_vec()
        })
    );
}

use serde::Deserialize;
use serde_json::{Value, json};

/// Complete recorded capture input and outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OscCase {
    /// Prefix and raw command bytes.
    input: Vec<u8>,
    /// Constructor allocation availability.
    dynamic: bool,
    /// Actual final byte, absent for ST.
    term: Option<u8>,
    /// Flat source union tag, when recorded separately.
    tag: Option<String>,
    /// Complete source payload or nested union.
    value: Option<Value>,
}

#[test]
fn osc_decoders_keep_returned_commands_across_reset() {
    let cases: Vec<OscCase> =
        serde_json::from_str(include_str!("fixtures/terminal-parsing.json")).unwrap();
    assert!(!cases.is_empty());
    for (index, row) in cases.into_iter().enumerate() {
        let mut parser = Parser::init(row.dynamic);
        for byte in &row.input {
            parser.next(*byte);
        }
        let command = parser.end(row.term);
        parser.reset();
        drop(parser);
        let actual = command.map_or(Value::Null, super::support::parsing::command);
        let expected = match row.tag {
            Some(tag) => json!({tag:row.value.unwrap_or_else(|| json!({}))}),
            None => row.value.unwrap_or(Value::Null),
        };
        assert_eq!(actual, expected, "OSC case {index}");
    }
    assert_eq!(super::support::parsing::terminator_bytes(None), b"\x1b\\");
    assert_eq!(super::support::parsing::terminator_bytes(Some(7)), b"\x07");
    assert_eq!(
        super::support::parsing::terminator_bytes(Some(27)),
        b"\x1b\\"
    );
    let mut parser = Parser::init(true);
    for byte in b"999;0;title" {
        parser.next(*byte);
    }
    assert_eq!(parser.end(None), None);
    parser.reset();
    for byte in b"2;reused" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(None),
        Some(Command::ChangeWindowTitle(b"reused".to_vec()))
    );
}

#[test]
fn decode_osc_0_longer_than_buffer() {
    let title = vec![b'x'; 2049];
    let input = [b"0;".as_slice(), &title].concat();
    for alloc in [false, true] {
        assert_eq!(decode(&input, alloc, None), None);
    }
}

#[test]
fn decode_osc_0_one_shorter_than_buffer_length() {
    let title = vec![b'x'; 2047];
    let input = [b"0;".as_slice(), &title].concat();
    for alloc in [false, true] {
        assert_eq!(
            decode(&input, alloc, None),
            Some(Command::ChangeWindowTitle(title.clone()))
        );
    }
}

#[test]
fn decode_osc_0_exactly_at_buffer_length() {
    let title = vec![b'x'; 2048];
    let input = [b"0;".as_slice(), &title].concat();
    for alloc in [false, true] {
        assert_eq!(decode(&input, alloc, None), None);
    }
}

#[test]
fn decode_osc_2_change_window_title_with_2() {
    assert_eq!(
        decode(b"2;different", false, None),
        Some(Command::ChangeWindowTitle(b"different".to_vec()))
    );
}

#[test]
fn decode_osc_2_change_window_title_with_utf8() {
    assert_eq!(
        decode("2;東京 résumé".as_bytes(), false, None),
        Some(Command::ChangeWindowTitle(
            "東京 résumé".as_bytes().to_vec()
        ))
    );
}

#[test]
fn decode_osc_2_change_window_title_empty() {
    assert_eq!(
        decode(b"2;", false, None),
        Some(Command::ChangeWindowTitle(b"".to_vec()))
    );
}

#[test]
fn decode_osc_52_get_set_clipboard() {
    assert_eq!(
        decode(b"52;s;?", false, None),
        Some(Command::ClipboardContents {
            kind: b's',
            data: b"?".to_vec()
        })
    );
}

#[test]
fn decode_osc_52_get_set_clipboard_with_allocator() {
    assert_eq!(
        decode(b"52;p;payload", true, None),
        Some(Command::ClipboardContents {
            kind: b'p',
            data: b"payload".to_vec()
        })
    );
}

#[test]
fn decode_osc_52_clear_clipboard() {
    assert_eq!(
        decode(b"52;;", false, None),
        Some(Command::ClipboardContents {
            kind: b'c',
            data: b"".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink() {
    assert_eq!(
        decode(b"8;;plain-uri", false, None),
        Some(Command::HyperlinkStart {
            id: None,
            uri: b"plain-uri".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_empty_id() {
    assert_eq!(
        decode(b"8;id=;empty-id", false, None),
        Some(Command::HyperlinkStart {
            id: None,
            uri: b"empty-id".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_incomplete_key() {
    assert_eq!(
        decode(b"8;id;incomplete", false, None),
        Some(Command::HyperlinkStart {
            id: None,
            uri: b"incomplete".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_empty_key() {
    assert_eq!(
        decode(b"8;=foo;empty-key", false, None),
        Some(Command::HyperlinkStart {
            id: None,
            uri: b"empty-key".to_vec()
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_empty_key_and_id() {
    assert_eq!(
        decode(b"8;=foo:id=;empty-both", false, None),
        Some(Command::HyperlinkStart {
            id: None,
            uri: b"empty-both".to_vec()
        })
    );
    assert_eq!(
        decode(b"8;=value:id=foo;http://example.com", false, None),
        Some(Command::HyperlinkStart {
            id: Some(b"foo".to_vec()),
            uri: b"http://example.com".to_vec(),
        })
    );
}

#[test]
fn decode_osc_8_hyperlink_with_empty_uri() {
    assert_eq!(decode(b"8;id=foo;", false, None), None);
}

#[test]
fn decode_osc_8_hyperlink_end() {
    assert_eq!(decode(b"8;;", false, None), Some(Command::HyperlinkEnd));
}

#[test]
fn decode_osc_1337_test_copy_with_no_value() {
    assert_eq!(decode(b"1337;Copy", false, None), None);
}

#[test]
fn decode_osc_1337_test_copy_with_empty_value() {
    assert_eq!(decode(b"1337;Copy=", false, None), None);
}

#[test]
fn decode_osc_1337_test_copy_with_only_prefix_colon() {
    assert_eq!(decode(b"1337;Copy=:", false, None), None);
}

#[test]
fn decode_osc_1337_test_copy_with_question_mark() {
    assert_eq!(decode(b"1337;Copy=:?", false, None), None);
}

#[test]
fn decode_osc_1337_test_copy_with_non_empty_value_that_is_valid_base64_but_not_prefixed_with_a_colon()
 {
    assert_eq!(decode(b"1337;Copy=aGVsbG8=", false, None), None);
}

#[test]
fn decode_osc_1337_test_copy_with_non_empty_value_that_is_invalid_base64() {
    assert_eq!(
        decode(b"1337;Copy=:notbase64", false, None),
        Some(Command::ClipboardContents {
            kind: b'c',
            data: b"notbase64".to_vec()
        })
    );
}

#[test]
fn decode_osc_1337_test_copy_with_non_empty_value_that_is_valid_base64() {
    assert_eq!(
        decode(b"1337;Copy=:aGVsbG8=", false, None),
        Some(Command::ClipboardContents {
            kind: b'c',
            data: b"aGVsbG8=".to_vec()
        })
    );
}

#[test]
fn decode_osc_1337_test_currentdir_with_no_value() {
    assert_eq!(decode(b"1337;CurrentDir", false, None), None);
}

#[test]
fn decode_osc_1337_test_currentdir_with_empty_value() {
    assert_eq!(decode(b"1337;CurrentDir=", false, None), None);
}

#[test]
fn directory_prefix_without_path_is_ignored() {
    assert_eq!(decode(b"9;9", false, None), None);
}

#[test]
fn decode_osc_7_report_pwd_empty() {
    assert_eq!(
        decode(b"7;", false, None),
        Some(Command::ReportPwd { value: Vec::new() })
    );
}

use maestro_terminal::osc::semantic_prompt::{
    Action as PromptAction, Option as PromptOption, OptionValue, PromptKind, Redraw,
};

/// Compare a prompt phase and all its independently decoded options.
fn assert_prompt(
    input: &[u8],
    action: PromptAction,
    options: &[u8],
    values: (Option<PromptKind>, Option<Redraw>),
) {
    let Some(Command::SemanticPrompt(command)) = decode(input, false, None) else {
        panic!("missing semantic command");
    };
    assert_eq!(command.action, action);
    assert_eq!(command.options_unvalidated, options);
    assert_eq!(
        command.read_option(PromptOption::PromptKind),
        values.0.map(OptionValue::PromptKind)
    );
    assert_eq!(
        command.read_option(PromptOption::Redraw),
        values.1.map(OptionValue::Redraw)
    );
}

#[test]
fn decode_osc_9_12_conemu_mark_prompt_start_1() {
    assert_prompt(b"9;12", PromptAction::FreshLineNewPrompt, b"", (None, None));
}

#[test]
fn decode_osc_9_12_conemu_mark_prompt_start_2() {
    assert_prompt(
        b"9;12;extra",
        PromptAction::FreshLineNewPrompt,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_end_input_start_output() {
    assert_prompt(
        b"133;C",
        PromptAction::EndInputStartOutput,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_end_input_start_output_extra_contents() {
    assert_eq!(decode(b"133;Cbad", false, None), None);
}

#[test]
fn decode_osc_133_end_input_start_output_with_options() {
    assert_prompt(
        b"133;C;raw=value",
        PromptAction::EndInputStartOutput,
        b"raw=value",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line() {
    assert_prompt(b"133;L", PromptAction::FreshLine, b"", (None, None));
}

#[test]
fn decode_osc_133_fresh_line_extra_contents() {
    assert_eq!(decode(b"133;Lbad", false, None), None);
}

#[test]
fn decode_osc_133_fresh_line_new_prompt() {
    assert_prompt(
        b"133;A",
        PromptAction::FreshLineNewPrompt,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_prompt_start() {
    assert_prompt(b"133;P", PromptAction::PromptStart, b"", (None, None));
}

#[test]
fn decode_osc_133_prompt_start_extra_contents() {
    assert_eq!(decode(b"133;Pbad", false, None), None);
}

#[test]
fn decode_osc_133_new_command() {
    assert_prompt(b"133;N", PromptAction::NewCommand, b"", (None, None));
}

#[test]
fn decode_osc_133_new_command_extra_contents() {
    assert_eq!(decode(b"133;Nbad", false, None), None);
}

#[test]
fn decode_osc_133_end_prompt_start_input() {
    assert_prompt(
        b"133;B",
        PromptAction::EndPromptStartInput,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_end_prompt_start_input_extra_contents() {
    assert_eq!(decode(b"133;Bbad", false, None), None);
}

#[test]
fn decode_osc_133_end_prompt_start_input_with_options() {
    assert_prompt(
        b"133;B;raw=value",
        PromptAction::EndPromptStartInput,
        b"raw=value",
        (None, None),
    );
}

#[test]
fn decode_osc_133_end_prompt_start_input_terminate_eol() {
    assert_prompt(
        b"133;I",
        PromptAction::EndPromptStartInputTerminateEol,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_end_prompt_start_input_terminate_eol_extra_contents() {
    assert_eq!(decode(b"133;Ibad", false, None), None);
}

#[test]
fn decode_osc_133_end_prompt_start_input_terminate_eol_with_options() {
    assert_prompt(
        b"133;I;raw=value",
        PromptAction::EndPromptStartInputTerminateEol,
        b"raw=value",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_trailing() {
    assert_prompt(
        b"133;A;",
        PromptAction::FreshLineNewPrompt,
        b"",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_bare_key() {
    assert_prompt(
        b"133;A;unknown",
        PromptAction::FreshLineNewPrompt,
        b"unknown",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_multiple_options() {
    assert_prompt(
        b"133;A;aid=14;cl=line",
        PromptAction::FreshLineNewPrompt,
        b"aid=14;cl=line",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_default_redraw() {
    assert_prompt(
        b"133;A;redraw=invalid",
        PromptAction::FreshLineNewPrompt,
        b"redraw=invalid",
        (None, None),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_redraw_0() {
    assert_prompt(
        b"133;A;redraw=0",
        PromptAction::FreshLineNewPrompt,
        b"redraw=0",
        (None, Some(Redraw::False)),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_redraw_1() {
    assert_prompt(
        b"133;A;redraw=1",
        PromptAction::FreshLineNewPrompt,
        b"redraw=1",
        (None, Some(Redraw::True)),
    );
}

#[test]
fn decode_osc_133_fresh_line_new_prompt_with_invalid_redraw() {
    assert_prompt(
        b"133;A;redraw=TRUE",
        PromptAction::FreshLineNewPrompt,
        b"redraw=TRUE",
        (None, None),
    );
}

#[test]
fn decode_osc_133_prompt_start_with_k_i() {
    assert_prompt(
        b"133;P;k=i",
        PromptAction::PromptStart,
        b"k=i",
        (Some(PromptKind::Initial), None),
    );
}

#[test]
fn decode_osc_133_prompt_start_with_k_c() {
    assert_prompt(
        b"133;P;k=c",
        PromptAction::PromptStart,
        b"k=c",
        (Some(PromptKind::Continuation), None),
    );
}

#[test]
fn decode_osc_133_prompt_start_with_k_s() {
    assert_prompt(
        b"133;P;k=s",
        PromptAction::PromptStart,
        b"k=s",
        (Some(PromptKind::Secondary), None),
    );
}

#[test]
fn decode_osc_133_prompt_start_with_invalid_k() {
    assert_prompt(
        b"133;P;k=x",
        PromptAction::PromptStart,
        b"k=x",
        (None, None),
    );
}

#[test]
fn decode_osc_133_new_command_with_multiple_options() {
    assert_prompt(
        b"133;N;first=1;second=2",
        PromptAction::NewCommand,
        b"first=1;second=2",
        (None, None),
    );
}

use maestro_terminal::{
    color::{Dynamic, Rgb, Special},
    osc::{
        Terminator,
        color::{ColoredTarget, Operation, Request, Target},
    },
};

/// Check a complete colour command with its ordered requests.
fn assert_color(input: &[u8], op: Operation, requests: Vec<Request>) {
    assert_eq!(
        decode(input, true, Some(7)),
        Some(Command::ColorOperation {
            op,
            requests,
            terminator: Terminator::Bel
        })
    );
}

#[test]
fn decode_osc_4_empty_param() {
    assert_eq!(decode(b"4;;", false, None), None);
}

#[test]
fn decode_osc_4() {
    let mut input = String::from("4");
    let mut requests = Vec::new();
    for index in 0..=255_u8 {
        use std::fmt::Write;
        write!(&mut input, ";{index};?").unwrap();
        requests.push(Request::Query(Target::Palette(index)));
    }
    for special in [
        Special::Bold,
        Special::Underline,
        Special::Blink,
        Special::Reverse,
        Special::Italic,
    ] {
        use std::fmt::Write;
        write!(&mut input, ";{};?", special.osc4()).unwrap();
        requests.push(Request::Query(Target::Special(special)));
    }
    assert_color(input.as_bytes(), Operation::Osc4, requests);
}

#[test]
fn decode_osc_5() {
    assert_color(
        b"5;0;?;1;?;2;?;3;?;4;?;5;?;0;red",
        Operation::Osc5,
        [
            Special::Bold,
            Special::Underline,
            Special::Blink,
            Special::Reverse,
            Special::Italic,
        ]
        .map(|s| Request::Query(Target::Special(s)))
        .to_vec(),
    );
}

#[test]
fn decode_osc_4_multiple_requests() {
    assert_color(
        b"4;3;red;1;?;3;blue;bad;?;2;?",
        Operation::Osc4,
        vec![
            Request::Set(ColoredTarget {
                target: Target::Palette(3),
                color: Rgb { r: 255, g: 0, b: 0 },
            }),
            Request::Query(Target::Palette(1)),
            Request::Set(ColoredTarget {
                target: Target::Palette(3),
                color: Rgb { r: 0, g: 0, b: 255 },
            }),
        ],
    );
}

#[test]
fn decode_osc_104() {
    assert_color(
        b"104;1;256;260",
        Operation::Osc104,
        vec![
            Request::Reset(Target::Palette(1)),
            Request::Reset(Target::Special(Special::Bold)),
            Request::Reset(Target::Special(Special::Italic)),
        ],
    );
}

#[test]
fn decode_osc_104_empty_index() {
    assert_color(
        b"104;;3;;4;",
        Operation::Osc104,
        vec![
            Request::Reset(Target::Palette(3)),
            Request::Reset(Target::Palette(4)),
        ],
    );
}

#[test]
fn decode_osc_104_invalid_index() {
    assert_color(
        b"104;bad;5;999;6",
        Operation::Osc104,
        vec![
            Request::Reset(Target::Palette(5)),
            Request::Reset(Target::Palette(6)),
        ],
    );
}

#[test]
fn decode_osc_104_reset_all() {
    assert_color(b"104;bad", Operation::Osc104, vec![Request::ResetPalette]);
}

#[test]
fn decode_osc_105_reset_all() {
    assert_color(b"105", Operation::Osc105, vec![Request::ResetSpecial]);
}

#[test]
fn decode_osc_10_osc_11_osc_12_osc_13_osc_14_osc_15_osc_16_osc_17_osc_18_osc_19_dynamic() {
    for (number, target) in DYNAMIC_TARGETS.into_iter().enumerate() {
        let input = format!("{};?", number + 10);
        let Some(Command::ColorOperation {
            op,
            requests,
            terminator,
        }) = decode(input.as_bytes(), true, None)
        else {
            panic!("missing dynamic command");
        };
        assert_eq!(op as usize, number + 10);
        assert_eq!(requests, vec![Request::Query(Target::Dynamic(target))]);
        assert_eq!(terminator, Terminator::St);
    }
}

#[test]
fn decode_osc_10_osc_11_osc_12_osc_13_osc_14_osc_15_osc_16_osc_17_osc_18_osc_19_dynamic_multiple() {
    for (number, _) in DYNAMIC_TARGETS.into_iter().enumerate() {
        let input = format!("{};{}", number + 10, ["?"; 11].join(";"));
        let Some(Command::ColorOperation { requests, .. }) = decode(input.as_bytes(), true, None)
        else {
            panic!("missing dynamic command");
        };
        assert_eq!(
            requests,
            DYNAMIC_TARGETS[number..]
                .iter()
                .map(|t| Request::Query(Target::Dynamic(*t)))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn decode_osc_110_osc_111_osc_112_osc_113_osc_114_osc_115_osc_116_osc_117_osc_118_osc_119_reset_dynamic()
 {
    for (number, target) in DYNAMIC_TARGETS.into_iter().enumerate() {
        let input = format!("{};", number + 110);
        let Some(Command::ColorOperation { op, requests, .. }) =
            decode(input.as_bytes(), true, None)
        else {
            panic!("missing dynamic reset");
        };
        assert_eq!(op as usize, number + 110);
        assert_eq!(requests, vec![Request::Reset(Target::Dynamic(target))]);
        let invalid = format!("{};extra", number + 110);
        let Some(Command::ColorOperation { requests, .. }) = decode(invalid.as_bytes(), true, None)
        else {
            panic!("missing dynamic reset command");
        };
        assert!(requests.is_empty());
    }
}

/// Dynamic colours in the independently recorded numeric order.
const DYNAMIC_TARGETS: [Dynamic; 10] = [
    Dynamic::Foreground,
    Dynamic::Background,
    Dynamic::Cursor,
    Dynamic::PointerForeground,
    Dynamic::PointerBackground,
    Dynamic::TektronixForeground,
    Dynamic::TektronixBackground,
    Dynamic::HighlightBackground,
    Dynamic::TektronixCursor,
    Dynamic::HighlightForeground,
];

#[test]
fn osc_capture_modes_and_terminators_match() {
    for (term, expected) in [
        (None, b"\x1b\\".as_slice()),
        (Some(7), b"\x07"),
        (Some(0), b"\x1b\\"),
    ] {
        assert_eq!(Terminator::init(term).string(), expected);
    }
    let payload = vec![b'z'; 3000];
    let input = [b"52;s;".as_slice(), &payload].concat();
    assert_eq!(decode(&input, false, None), None);
    assert_eq!(
        decode(&input, true, None),
        Some(Command::ClipboardContents {
            kind: b's',
            data: payload
        })
    );
    let mut parser = Parser::init(true);
    for byte in b"000;2;ignored" {
        parser.next(*byte);
    }
    assert_eq!(parser.end(None), None);
    parser.reset();
    for byte in b"2;after reset" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(None),
        Some(Command::ChangeWindowTitle(b"after reset".to_vec()))
    );
}

#[test]
fn decode_osc_21_kitty_color_protocol() {
    use maestro_terminal::kitty::color::{Kind, Osc, Request as K, Special as S};
    assert_eq!(
        decode(b"21;foreground=?;2=#123;cursor=", true, Some(7)),
        Some(Command::KittyColorProtocol(Osc {
            list: vec![
                K::Query(Kind::Special(S::Foreground)),
                K::Set {
                    key: Kind::Palette(2),
                    color: Rgb {
                        r: 17,
                        g: 34,
                        b: 51
                    }
                },
                K::Reset(Kind::Special(S::Cursor))
            ],
            terminator: Terminator::Bel,
        }))
    );
}

#[test]
fn decode_osc_21_kitty_color_protocol_without_allocator() {
    assert_eq!(decode(b"21;foreground=?", false, Some(7)), None);
}

#[test]
fn decode_osc_21_kitty_color_protocol_reset_after_invalid() {
    use maestro_terminal::kitty::color::{Kind, Request as K};
    let Some(Command::KittyColorProtocol(command)) =
        decode(b"21;invalid=?;1=invalid-colour;2=", true, None)
    else {
        panic!("missing protocol command");
    };
    assert_eq!(command.list, vec![K::Reset(Kind::Palette(2))]);
    assert_eq!(command.terminator, Terminator::St);
}

#[test]
fn decode_osc_21_kitty_color_protocol_no_key() {
    let Some(Command::KittyColorProtocol(command)) = decode(b"21;=red", true, None) else {
        panic!("missing empty protocol command");
    };
    assert!(command.list.is_empty());
    assert_eq!(command.terminator, Terminator::St);
}

#[test]
fn decode_option_read_prompt_kind() {
    for (raw, expected) in [
        (b"k=i".as_slice(), Some(PromptKind::Initial)),
        (b"k=r", Some(PromptKind::Right)),
        (b"k=c", Some(PromptKind::Continuation)),
        (b"k=s", Some(PromptKind::Secondary)),
        (b"k=", None),
        (b"k=ir", None),
        (b"K=i", None),
        (b"k=x", None),
        (b"k", None),
        (b"", None),
    ] {
        assert_eq!(
            PromptOption::PromptKind.read(raw),
            expected.map(OptionValue::PromptKind)
        );
    }
}

#[test]
fn decode_option_read_redraw() {
    for (raw, expected) in [
        (b"redraw=0".as_slice(), Some(Redraw::False)),
        (b"redraw=1", Some(Redraw::True)),
        (b"redraw=last", Some(Redraw::Last)),
        (b"redraw=", None),
        (b"redraw=2", None),
        (b"redraw=Last", None),
        (b"Redraw=1", None),
        (b"redraw", None),
        (b"", None),
    ] {
        assert_eq!(
            PromptOption::Redraw.read(raw),
            expected.map(OptionValue::Redraw)
        );
    }
}

#[test]
fn decode_osc_kitty_color_protocol_kind_string() {
    use maestro_terminal::kitty::color::{Kind, Special as S};
    assert_eq!(Kind::Special(S::Foreground).to_string(), "foreground");
    assert_eq!(Kind::Palette(42).to_string(), "42");
    for index in 0..=255 {
        assert!(Kind::Palette(index).has_terminal_query_color());
    }
    for (special, queryable) in [
        (S::Foreground, true),
        (S::Background, true),
        (S::Cursor, true),
        (S::SelectionForeground, false),
        (S::SelectionBackground, false),
        (S::CursorText, false),
        (S::VisualBell, false),
        (S::SecondTransparentBackground, false),
    ] {
        assert_eq!(Kind::Special(special).has_terminal_query_color(), queryable);
    }
}

#[test]
fn colour_protocol_keys_and_request_limit_match() {
    use maestro_terminal::kitty::color::{Kind, Request as K, Special as S};
    for (name, special) in [
        ("foreground", S::Foreground),
        ("background", S::Background),
        ("selection_foreground", S::SelectionForeground),
        ("selection_background", S::SelectionBackground),
        ("cursor", S::Cursor),
        ("cursor_text", S::CursorText),
        ("visual_bell", S::VisualBell),
        (
            "second_transparent_background",
            S::SecondTransparentBackground,
        ),
    ] {
        assert_eq!(Kind::parse(name.as_bytes()), Some(Kind::Special(special)));
        assert_eq!(Kind::Special(special).to_string(), name);
    }
    assert_eq!(Kind::parse(b"0"), Some(Kind::Palette(0)));
    assert_eq!(Kind::parse(b"255"), Some(Kind::Palette(255)));
    for invalid in [
        b"256".as_slice(),
        b"-1",
        b"+1",
        b"0x1",
        b"Foreground",
        b" foreground",
        b"foreground ",
        b"",
    ] {
        assert_eq!(Kind::parse(invalid), None);
    }
    for count in [525, 526] {
        for suffix in ["", ";", ";unknown", ";1"] {
            let input = format!("21;{}{suffix}", vec!["1"; count].join(";"));
            let result = decode(input.as_bytes(), true, None);
            if count == 526 && !suffix.is_empty() {
                assert_eq!(result, None);
                continue;
            }
            let Some(Command::KittyColorProtocol(command)) = result else {
                panic!("missing bounded list");
            };
            assert_eq!(
                command.list,
                vec![K::Reset(Kind::Palette(1)); count + usize::from(suffix == ";1")]
            );
            assert_eq!(command.terminator, Terminator::St);
        }
    }
}

#[test]
fn semantic_options_keep_first_match_and_raw_bytes() {
    for (letter, action) in [
        (b'A', PromptAction::FreshLineNewPrompt),
        (b'N', PromptAction::NewCommand),
        (b'P', PromptAction::PromptStart),
        (b'B', PromptAction::EndPromptStartInput),
        (b'I', PromptAction::EndPromptStartInputTerminateEol),
        (b'C', PromptAction::EndInputStartOutput),
        (b'D', PromptAction::EndCommand),
    ] {
        let raw = b"k=bad;k=i;redraw=1;redraw=0;unknown=\xff";
        let input = [b"133;".as_slice(), &[letter], b";", raw].concat();
        assert_prompt(&input, action, raw, (None, Some(Redraw::True)));
        assert_eq!(
            decode(
                &[b"133;".as_slice(), &[letter], b"bad"].concat(),
                false,
                None
            ),
            None
        );
    }
    assert_prompt(b"133;L", PromptAction::FreshLine, b"", (None, None));
    assert_eq!(decode(b"133;L;", false, None), None);
    assert_prompt(
        b"133;P;k=s;k=r;redraw=bad;redraw=last",
        PromptAction::PromptStart,
        b"k=s;k=r;redraw=bad;redraw=last",
        (Some(PromptKind::Secondary), None),
    );
}

#[test]
fn osc_byte_payloads_are_owned_not_utf8_narrowed() {
    let cases = [
        (
            b"0;\xfftitle".as_slice(),
            Command::ChangeWindowTitle(b"\xfftitle".to_vec()),
        ),
        (
            b"7;\xffdir",
            Command::ReportPwd {
                value: b"\xffdir".to_vec(),
            },
        ),
        (
            b"8;id=\xffid;\xffuri",
            Command::HyperlinkStart {
                id: Some(b"\xffid".to_vec()),
                uri: b"\xffuri".to_vec(),
            },
        ),
        (
            b"52;s;\xffdata",
            Command::ClipboardContents {
                kind: b's',
                data: b"\xffdata".to_vec(),
            },
        ),
    ];
    for (input, expected) in cases {
        let mut parser = Parser::init(false);
        let input = input.to_vec();
        for byte in &input {
            parser.next(*byte);
        }
        let command = parser.end(None);
        drop(input);
        parser.reset();
        for byte in b"2;replacement" {
            parser.next(*byte);
        }
        assert_eq!(
            parser.end(None),
            Some(Command::ChangeWindowTitle(b"replacement".to_vec()))
        );
        drop(parser);
        assert_eq!(
            decode(b"2;second instance", false, None),
            Some(Command::ChangeWindowTitle(b"second instance".to_vec()))
        );
        assert_eq!(command, Some(expected));
    }
}

#[test]
fn osc_requests_keep_order_and_partial_results() {
    assert_color(
        b"4;2;?;1;red;2;blue;bad;?;3;?",
        Operation::Osc4,
        vec![
            Request::Query(Target::Palette(2)),
            Request::Set(ColoredTarget {
                target: Target::Palette(1),
                color: Rgb { r: 255, g: 0, b: 0 },
            }),
            Request::Set(ColoredTarget {
                target: Target::Palette(2),
                color: Rgb { r: 0, g: 0, b: 255 },
            }),
        ],
    );
    assert_color(
        b"5;2;?;1;?;2;?;5;?;0;?",
        Operation::Osc5,
        vec![
            Request::Query(Target::Special(Special::Blink)),
            Request::Query(Target::Special(Special::Underline)),
            Request::Query(Target::Special(Special::Blink)),
        ],
    );
    assert_color(
        b"104;2;bad;1;2",
        Operation::Osc104,
        vec![
            Request::Reset(Target::Palette(2)),
            Request::Reset(Target::Palette(1)),
            Request::Reset(Target::Palette(2)),
        ],
    );
    assert_color(
        b"10;red;notacolor;blue",
        Operation::Osc10,
        vec![Request::Set(ColoredTarget {
            target: Target::Dynamic(Dynamic::Foreground),
            color: Rgb { r: 255, g: 0, b: 0 },
        })],
    );
    assert_color(
        b"19;?;red",
        Operation::Osc19,
        vec![Request::Query(Target::Dynamic(
            Dynamic::HighlightForeground,
        ))],
    );
    assert_color(b"112;?", Operation::Osc112, vec![]);
}

#[test]
fn osc_numeric_rejection_keeps_each_list_policy() {
    use maestro_terminal::kitty::color::{Kind, Request as K};
    assert_color(
        b"4;1;red;2_0;blue;2;red",
        Operation::Osc4,
        vec![Request::Set(ColoredTarget {
            target: Target::Palette(1),
            color: Rgb { r: 255, g: 0, b: 0 },
        })],
    );
    assert_color(
        b"104;1_0;2",
        Operation::Osc104,
        vec![Request::Reset(Target::Palette(2))],
    );
    assert_color(
        b"4;+1;?;-0;?;0x1;?;2;red",
        Operation::Osc4,
        vec![
            Request::Query(Target::Palette(1)),
            Request::Query(Target::Palette(0)),
        ],
    );
    let Some(Command::KittyColorProtocol(command)) =
        decode(b"21;+1=red;1_0=red;1=rgbi:0_1/0/0;2=red", true, None)
    else {
        panic!("missing continuing protocol list");
    };
    assert_eq!(
        command.list,
        vec![K::Set {
            key: Kind::Palette(2),
            color: Rgb { r: 255, g: 0, b: 0 }
        }]
    );
}

#[test]
fn osc_colour_token_walk_is_single_pass() {
    for pairs in [64, 128] {
        let input = format!("4;{}", vec!["2;?"; pairs].join(";"));
        assert_color(
            input.as_bytes(),
            Operation::Osc4,
            vec![Request::Query(Target::Palette(2)); pairs],
        );
    }
}

/// Full recorded key parsing, formatting and queryability observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyCase {
    /// Key parser operation.
    kind: String,
    /// Authored key bytes.
    input: Vec<u8>,
    /// Complete parsed key, including failure.
    value: Value,
    /// Queryability of a successful key.
    queryable: bool,
    /// Exact formatting bytes of a successful key.
    formatted: Vec<u8>,
}

#[test]
fn colour_key_corpus_preserves_failure_and_identity() {
    use maestro_terminal::kitty::color::Kind;
    let rows: Vec<KeyCase> =
        serde_json::from_str(include_str!("fixtures/colour-keys.json")).unwrap();
    for row in rows {
        assert_eq!(row.kind, "kitty_kind");
        let parsed = Kind::parse(&row.input);
        let actual = parsed.map_or(Value::Null, |key| match key {
            Kind::Palette(index) => json!({"palette":index}),
            Kind::Special(special) => {
                json!({"special":super::support::parsing::wire_name(special)})
            }
        });
        assert_eq!(actual, row.value);
        assert_eq!(
            parsed.is_some_and(Kind::has_terminal_query_color),
            row.queryable
        );
        assert_eq!(
            parsed
                .map(|k| k.to_string().into_bytes())
                .unwrap_or_default(),
            row.formatted
        );
    }
}

#[test]
fn rejected_capture_cannot_resume_before_reset() {
    for (prefix, suffix) in [
        (b"0".as_slice(), b";late".as_slice()),
        (b"52;bad", b";late"),
        (b"8;id=x;", b"late"),
        (b"133;", b"A"),
    ] {
        let mut parser = Parser::init(false);
        for byte in prefix {
            parser.next(*byte);
        }
        assert_eq!(parser.end(None), None);
        for byte in suffix {
            parser.next(*byte);
        }
        assert_eq!(parser.end(None), None);
        parser.reset();
        for byte in b"2;fresh" {
            parser.next(*byte);
        }
        assert_eq!(
            parser.end(None),
            Some(Command::ChangeWindowTitle(b"fresh".to_vec()))
        );
    }
}

#[test]
fn public_prompt_decoder_keeps_failed_capture_invalid() {
    let mut parser = Parser::init(false);
    for byte in b"133;" {
        parser.next(*byte);
    }
    assert_eq!(parser.end(None), None);
    parser.next(b'A');
    assert_eq!(parser.end(None), None);
    parser.reset();
    for byte in b"133;A" {
        parser.next(*byte);
    }
    let expected =
        maestro_terminal::osc::semantic_prompt::Command::init(PromptAction::FreshLineNewPrompt);
    assert_eq!(parser.end(None), Some(Command::SemanticPrompt(expected)));
}

#[test]
fn public_colour_decoder_rejects_wrong_prefix_before_reuse() {
    let mut parser = Parser::init(true);
    parser.next(b'1');
    assert_eq!(parser.end(None), None);
    for byte in b"0;?" {
        parser.next(*byte);
    }
    assert_eq!(parser.end(None), None);
    parser.reset();
    for byte in b"10;?" {
        parser.next(*byte);
    }
    assert_eq!(
        parser.end(None),
        Some(Command::ColorOperation {
            op: Operation::Osc10,
            requests: vec![Request::Query(Target::Dynamic(Dynamic::Foreground))],
            terminator: Terminator::St,
        })
    );
}

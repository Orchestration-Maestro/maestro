//! Typed actions observed at the byte-stream boundary.

use maestro_terminal::stream::{Action, Handler, Stream};

#[derive(Default)]
struct Recorder(Vec<char>);

impl Handler for Recorder {
    fn vt(&mut self, action: Action) {
        if let Action::Print(cp) = action {
            self.0.push(cp);
        }
    }
}

struct Expected<'a> {
    actions: &'a [Action<'a>],
    seen: usize,
}

impl Handler for Expected<'_> {
    fn vt(&mut self, action: Action) {
        assert_eq!(self.actions.get(self.seen), Some(&action));
        self.seen += 1;
    }
}

fn expect(bytes: &[u8], actions: &[Action]) {
    let mut expected = Expected { actions, seen: 0 };
    Stream::new(false).next_slice(bytes, &mut expected);
    assert_eq!(expected.seen, actions.len());
}

#[test]
fn ground_controls_keep_the_decoding_boundary() {
    expect(
        b"\0\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f",
        &[
            Action::Enquiry,
            Action::Bell,
            Action::Backspace,
            Action::HorizontalTab(1),
            Action::Linefeed,
            Action::Linefeed,
            Action::Linefeed,
            Action::CarriageReturn,
            Action::InvokeCharset {
                bank: maestro_terminal::CharsetActiveSlot::Gl,
                charset: maestro_terminal::CharsetSlot::G1,
                locking: false,
            },
            Action::InvokeCharset {
                bank: maestro_terminal::CharsetActiveSlot::Gl,
                charset: maestro_terminal::CharsetSlot::G0,
                locking: false,
            },
        ],
    );
    let printed: Vec<_> = (0x10..=0x1f)
        .filter(|&cp| cp != 0x1b)
        .chain([0x7f, 0x85, 0x9b])
        .map(|cp| char::from_u32(cp).unwrap())
        .collect();
    let bytes: String = printed.iter().collect();
    expect(
        bytes.as_bytes(),
        &printed.into_iter().map(Action::Print).collect::<Vec<_>>(),
    );
    expect(b"\x1b[\x84", &[Action::Index]);
}

type CountAction = fn(u16) -> Action<'static>;

#[test]
fn count_commands_keep_defaults_and_rejections() {
    let commands: &[(u8, CountAction)] = &[
        (b'A', Action::CursorUp),
        (b'k', Action::CursorUp),
        (b'B', Action::CursorDown),
        (b'C', Action::CursorRight),
        (b'D', Action::CursorLeft),
        (b'j', Action::CursorLeft),
        (b'G', Action::CursorCol),
        (b'`', Action::CursorCol),
        (b'I', Action::HorizontalTab),
        (b'L', Action::InsertLines),
        (b'M', Action::DeleteLines),
        (b'P', Action::DeleteChars),
        (b'S', Action::ScrollUp),
        (b'T', Action::ScrollDown),
        (b'X', Action::EraseChars),
        (b'Z', Action::HorizontalTabBack),
        (b'a', Action::CursorColRelative),
        (b'b', Action::PrintRepeat),
        (b'd', Action::CursorRow),
        (b'e', Action::CursorRowRelative),
        (b'@', Action::InsertBlanks),
    ];
    for &(command, action) in commands {
        for (operand, value) in [("", 1), ("0", 0), ("1", 1), ("65535", 65535)] {
            let value = if matches!(command, b'@' | b'a' | b'e') {
                value.max(1)
            } else {
                value
            };
            expect(
                format!("\x1b[{operand}{}", char::from(command)).as_bytes(),
                &[action(value)],
            );
        }
        for operand in ["1;2", "?1", "1 ", "1:"] {
            expect(
                format!("\x1b[{operand}{}", char::from(command)).as_bytes(),
                &[],
            );
        }
    }
    for (command, action) in [
        (b'E', Action::CursorDown as fn(u16) -> Action<'static>),
        (b'F', Action::CursorUp),
    ] {
        for (operand, value) in [("", 1), ("0", 0), ("1", 1), ("65535", 65535)] {
            expect(
                format!("\x1b[{operand}{}", char::from(command)).as_bytes(),
                &[action(value), Action::CarriageReturn],
            );
        }
        expect(format!("\x1b[1;2{}", char::from(command)).as_bytes(), &[]);
        expect(format!("\x1b[?1{}", char::from(command)).as_bytes(), &[]);
    }
}

#[test]
fn dispatch_print() {
    let mut stream = Stream::new(false);
    let mut recorder = Recorder::default();
    stream.next_slice("hello 😀".as_bytes(), &mut recorder);
    assert_eq!(recorder.0.into_iter().collect::<String>(), "hello 😀");
}

#[test]
fn position_commands_keep_each_coordinate() {
    for command in ['H', 'f'] {
        for (operand, row, col) in [
            ("", 1, 1),
            ("0", 0, 1),
            ("3", 3, 1),
            ("0;0", 0, 0),
            ("3;8", 3, 8),
            ("65535;65535", 65535, 65535),
        ] {
            expect(
                format!("\x1b[{operand}{command}").as_bytes(),
                &[Action::CursorPos { row, col }],
            );
        }
        for operand in ["1;2;3", "?1", "1 "] {
            expect(format!("\x1b[{operand}{command}").as_bytes(), &[]);
        }
    }
}

#[test]
fn tab_controls_keep_each_admitted_form() {
    for bytes in [b"\x1b[W".as_slice(), b"\x1b[0W"] {
        expect(bytes, &[Action::TabSet]);
    }
    expect(b"\x1b[2W", &[Action::TabClearCurrent]);
    expect(b"\x1b[5W", &[Action::TabClearAll]);
    expect(b"\x1b[?5W", &[Action::TabReset]);
    expect(
        b"\x1b[g\x1b[0g\x1b[3g",
        &[
            Action::TabClearCurrent,
            Action::TabClearCurrent,
            Action::TabClearAll,
        ],
    );
    for command in ['W', 'g'] {
        for operand in ["1", "65535", "1;2", "?0", "?2", "0 "] {
            expect(format!("\x1b[{operand}{command}").as_bytes(), &[]);
        }
    }
    expect(b"\x1b[?W\x1b[?3g\x1b[?5g\x1b[3W", &[]);
}

#[test]
fn esc_dispatch_keeps_charsets_and_controls() {
    use maestro_terminal::{Charset, CharsetActiveSlot as Bank, CharsetSlot as Slot, Mode};
    for (marker, slot) in [
        ('(', Slot::G0),
        (')', Slot::G1),
        ('*', Slot::G2),
        ('+', Slot::G3),
    ] {
        for (code, charset) in [
            ('B', Charset::Ascii),
            ('A', Charset::British),
            ('0', Charset::DecSpecial),
        ] {
            expect(
                format!("\x1b{marker}{code}").as_bytes(),
                &[Action::ConfigureCharset { slot, charset }],
            );
            expect(format!("\x1b {marker}{code}").as_bytes(), &[]);
        }
    }
    for (code, bank, charset, locking) in [
        ('N', Bank::Gl, Slot::G2, true),
        ('O', Bank::Gl, Slot::G3, true),
        ('n', Bank::Gl, Slot::G2, false),
        ('o', Bank::Gl, Slot::G3, false),
        ('~', Bank::Gr, Slot::G1, false),
        ('}', Bank::Gr, Slot::G2, false),
        ('|', Bank::Gr, Slot::G3, false),
    ] {
        expect(
            format!("\x1b{code}").as_bytes(),
            &[Action::InvokeCharset {
                bank,
                charset,
                locking,
            }],
        );
        expect(format!("\x1b {code}").as_bytes(), &[]);
    }
    expect(
        b"\x1b7\x1b8\x1b#8\x1bD\x1bE\x1bH\x1bM\x1bV\x1bW\x1bZ\x1bc\x1b=\x1b>\x1b\\",
        &[
            Action::SaveCursor,
            Action::RestoreCursor,
            Action::Decaln,
            Action::Index,
            Action::NextLine,
            Action::TabSet,
            Action::ReverseIndex,
            Action::ProtectedModeIso,
            Action::ProtectedModeOff,
            Action::DeviceAttributes(maestro_terminal::device_attributes::Req::Primary),
            Action::FullReset,
            Action::SetMode(Mode::KeypadKeys),
            Action::ResetMode(Mode::KeypadKeys),
        ],
    );
    for code in ['7', '8', 'D', 'E', 'H', 'M', 'V', 'W', 'Z', 'c', '=', '>'] {
        expect(format!("\x1b {code}").as_bytes(), &[]);
    }
    expect(b"\x1bQ\x1b-B", &[]);
}

#[test]
fn erase_dispatch_keeps_protection_and_admitted_modes() {
    type EraseAction = fn(bool) -> Action<'static>;
    let display: &[(u16, EraseAction)] = &[
        (0, Action::EraseDisplayBelow),
        (1, Action::EraseDisplayAbove),
        (2, Action::EraseDisplayComplete),
        (3, Action::EraseDisplayScrollback),
        (22, Action::EraseDisplayScrollComplete),
    ];
    let line: &[(u16, EraseAction)] = &[
        (0, Action::EraseLineRight),
        (1, Action::EraseLineLeft),
        (2, Action::EraseLineComplete),
    ];
    for (code, modes) in [('J', display), ('K', line)] {
        for (marker, protected) in [("", false), ("?", true)] {
            for &(mode, action) in modes {
                expect(
                    format!("\x1b[{marker}{mode}{code}").as_bytes(),
                    &[action(protected)],
                );
            }
            expect(
                format!("\x1b[{marker}{code}").as_bytes(),
                &[modes[0].1(protected)],
            );
        }
        for operand in ["4", "65535", "0;0", ">0", "0 "] {
            expect(format!("\x1b[{operand}{code}").as_bytes(), &[]);
        }
    }
    expect(b"\x1b[3K\x1b[?3K\x1b[22K\x1b[5J", &[]);
}

#[test]
fn mode_lists_keep_namespace_order_and_unknowns() {
    use maestro_terminal::Mode;
    expect(
        b"\x1b[4;999;4;20h\x1b[4;999;20l\x1b[?7;999;6;7h\x1b[?6;999;7l\x1b[?7;999;6;7s\x1b[?6;999;7r",
        &[
            Action::SetMode(Mode::Insert), Action::SetMode(Mode::Insert),
            Action::SetMode(Mode::Linefeed), Action::ResetMode(Mode::Insert),
            Action::ResetMode(Mode::Linefeed), Action::SetMode(Mode::Wraparound),
            Action::SetMode(Mode::Origin), Action::SetMode(Mode::Wraparound),
            Action::ResetMode(Mode::Origin), Action::ResetMode(Mode::Wraparound),
            Action::SaveMode(Mode::Wraparound), Action::SaveMode(Mode::Origin),
            Action::SaveMode(Mode::Wraparound), Action::RestoreMode(Mode::Origin),
            Action::RestoreMode(Mode::Wraparound),
        ],
    );
    for final_byte in *b"hlsr" {
        for marker in [b">".as_slice(), b"=", b"??", b"? "] {
            let mut bytes = b"\x1b[".to_vec();
            bytes.extend(marker);
            bytes.extend(b"7");
            bytes.push(final_byte);
            expect(&bytes, &[]);
        }
    }
    expect(b"\x1b[h\x1b[l\x1b[?s\x1b[?r", &[]);
}

#[test]
fn mode_queries_dispatch_ansi_and_private_requests() {
    use maestro_terminal::{Mode, modes::ModeTag};
    expect(
        b"\x1b[4$p\x1b[999$p\x1b[?7$p\x1b[?999$p",
        &[
            Action::RequestMode(Mode::Insert),
            Action::RequestModeUnknown(ModeTag {
                value: 999,
                ansi: true,
            }),
            Action::RequestMode(Mode::Wraparound),
            Action::RequestModeUnknown(ModeTag {
                value: 999,
                ansi: false,
            }),
        ],
    );
    for bytes in [
        b"\x1b[$p".as_slice(),
        b"\x1b[4;7$p",
        b"\x1b[?4;7$p",
        b"\x1b[>4$p",
        b"\x1b[4p",
    ] {
        expect(bytes, &[]);
    }
}

#[test]
fn modifier_requests_keep_the_numeric_mode_distinction() {
    use maestro_terminal::ansi::ModifyKeyFormat as F;
    for (params, format) in [
        ("", F::Legacy),
        ("0", F::Legacy),
        ("1", F::CursorKeys),
        ("2", F::FunctionKeys),
        ("4", F::OtherKeysNone),
        ("4;2", F::OtherKeysNumeric),
        ("4;1", F::OtherKeysNone),
        ("0;2", F::Legacy),
        ("1;2", F::CursorKeys),
        ("2;2", F::FunctionKeys),
    ] {
        expect(
            format!("\x1b[>{params}m").as_bytes(),
            &[Action::ModifyKeyFormat(format)],
        );
    }
    for params in ["", "4", "4;2;9", "65535"] {
        expect(
            format!("\x1b[>{params}n").as_bytes(),
            &[Action::ModifyKeyFormat(F::OtherKeysNumericExcept)],
        );
    }
    for bytes in [b"\x1b[>3m".as_slice(), b"\x1b[>4;2;9m", b"\x1b[?4m"] {
        expect(bytes, &[]);
    }
}

#[test]
fn keyboard_dispatch_keeps_flag_and_count_domains() {
    use maestro_terminal::kitty::key::Flags;
    for value in 0..=31 {
        let flags = Flags::from_int(value).unwrap();
        expect(
            format!("\x1b[>{value}u").as_bytes(),
            &[Action::KittyKeyboardPush(flags)],
        );
        for (op, action) in [
            (1, Action::KittyKeyboardSet(flags)),
            (2, Action::KittyKeyboardSetOr(flags)),
            (3, Action::KittyKeyboardSetNot(flags)),
        ] {
            expect(format!("\x1b[={value};{op};9u").as_bytes(), &[action]);
        }
    }
    for params in ["", "32;7", "65535;2"] {
        expect(
            format!("\x1b[>{params}u").as_bytes(),
            &[Action::KittyKeyboardPush(Flags::DISABLED)],
        );
        expect(
            format!("\x1b[<{params}u").as_bytes(),
            &[Action::KittyKeyboardPop(1)],
        );
    }
    for params in ["", "0", "65535;2"] {
        expect(
            format!("\x1b[?{params}u").as_bytes(),
            &[Action::KittyKeyboardQuery],
        );
        expect(
            format!("\x1b[{params}u").as_bytes(),
            &[Action::RestoreCursor],
        );
    }
    for value in [0, 1, 65535] {
        expect(
            format!("\x1b[<{value}u").as_bytes(),
            &[Action::KittyKeyboardPop(value)],
        );
    }
    expect(b"\x1b[=u", &[Action::KittyKeyboardSet(Flags::DISABLED)]);
    expect(
        b"\x1b[=3u",
        &[Action::KittyKeyboardSet(Flags::from_int(3).unwrap())],
    );
    for bytes in [
        b"\x1b[>32u".as_slice(),
        b"\x1b[>65535u",
        b"\x1b[=32u",
        b"\x1b[=65535;2u",
        b"\x1b[=3;0u",
        b"\x1b[=3;4u",
    ] {
        expect(bytes, &[]);
    }
    assert_eq!(Flags::from_int(32), None);
    assert_eq!(Flags::from_int(65535), None);
}

#[test]
fn query_dispatch_keeps_request_kinds() {
    use maestro_terminal::{
        csi::SizeReportStyle as S, device_attributes::Req, device_status::Request as R,
    };
    for params in ["", "0", "65535;2"] {
        for (marker, req) in [
            ("", Req::Primary),
            (">", Req::Secondary),
            ("=", Req::Tertiary),
        ] {
            expect(
                format!("\x1b[{marker}{params}c").as_bytes(),
                &[Action::DeviceAttributes(req)],
            );
        }
        expect(format!("\x1b[>{params}q").as_bytes(), &[Action::Xtversion]);
    }
    expect(
        b"\x1b[5n\x1b[6n\x1b[?996n",
        &[
            Action::DeviceStatus(R::OperatingStatus),
            Action::DeviceStatus(R::CursorPosition),
            Action::DeviceStatus(R::ColorScheme),
        ],
    );
    for (number, style) in [
        (14, S::Csi14T),
        (16, S::Csi16T),
        (18, S::Csi18T),
        (21, S::Csi21T),
    ] {
        expect(
            format!("\x1b[{number}t").as_bytes(),
            &[Action::SizeReport(style)],
        );
        expect(format!("\x1b[{number};0t").as_bytes(), &[]);
        expect(format!("\x1b[?{number}t").as_bytes(), &[]);
    }
    for bytes in [
        b"\x1b[?c".as_slice(),
        b"\x1b[n",
        b"\x1b[5;6n",
        b"\x1b[?5n",
        b"\x1b[996n",
        b"\x1b[7n",
        b"\x1b[t",
        b"\x1b[15t",
    ] {
        expect(bytes, &[]);
    }
}

#[test]
fn status_and_shift_requests_keep_their_domains() {
    use maestro_terminal::ansi::{CursorStyle as C, StatusDisplay as D};
    expect(
        b"\x1b[0$}\x1b[1$}\x1b[>s\x1b[>0s\x1b[>1s",
        &[
            Action::ActiveStatusDisplay(D::Main),
            Action::ActiveStatusDisplay(D::StatusLine),
            Action::MouseShiftCapture(false),
            Action::MouseShiftCapture(false),
            Action::MouseShiftCapture(true),
        ],
    );
    for (number, style) in [
        (0, C::Default),
        (1, C::BlinkingBlock),
        (2, C::SteadyBlock),
        (3, C::BlinkingUnderline),
        (4, C::SteadyUnderline),
        (5, C::BlinkingBar),
        (6, C::SteadyBar),
    ] {
        expect(
            format!("\x1b[{number} q").as_bytes(),
            &[Action::CursorStyle(style)],
        );
    }
    expect(b"\x1b[ q", &[Action::CursorStyle(C::Default)]);
    expect(
        b"\x1b[\"q\x1b[0\"q\x1b[1\"q\x1b[2\"q",
        &[
            Action::ProtectedModeOff,
            Action::ProtectedModeOff,
            Action::ProtectedModeDec,
            Action::ProtectedModeOff,
        ],
    );
    for bytes in [
        b"\x1b[$}".as_slice(),
        b"\x1b[2$}",
        b"\x1b[0;1$}",
        b"\x1b[0}",
        b"\x1b[>2s",
        b"\x1b[>0;1s",
        b"\x1b[q",
        b"\x1b[7 q",
        b"\x1b[1;2 q",
        b"\x1b[3\"q",
        b"\x1b[0;1\"q",
    ] {
        expect(bytes, &[]);
    }
}

#[test]
fn title_requests_keep_window_index_rules() {
    for (number, action) in [
        (22, Action::TitlePush as CountAction),
        (23, Action::TitlePop),
    ] {
        for selection in [0, 2] {
            expect(
                format!("\x1b[{number};{selection}t").as_bytes(),
                &[action(0)],
            );
            for index in [0, 1, 65535] {
                expect(
                    format!("\x1b[{number};{selection};{index}t").as_bytes(),
                    &[action(index)],
                );
            }
        }
        for params in ["", ";1", ";3", ";0;1;2"] {
            expect(format!("\x1b[{number}{params}t").as_bytes(), &[]);
        }
        expect(format!("\x1b[?{number};0t").as_bytes(), &[]);
    }
}

#[test]
fn margin_requests_keep_counts_and_ambiguity() {
    for (params, top_left, bottom_right) in [
        ("", 0, 0),
        ("0", 0, 0),
        ("3", 3, 0),
        ("3;8", 3, 8),
        ("0;65535", 0, 65535),
    ] {
        expect(
            format!("\x1b[{params}r").as_bytes(),
            &[Action::TopAndBottomMargin {
                top_left,
                bottom_right,
            }],
        );
        if params.is_empty() {
            expect(b"\x1b[s", &[Action::LeftAndRightMarginAmbiguous]);
        } else {
            expect(
                format!("\x1b[{params}s").as_bytes(),
                &[Action::LeftAndRightMargin {
                    top_left,
                    bottom_right,
                }],
            );
        }
    }
    for bytes in [
        b"\x1b[1;2;3r".as_slice(),
        b"\x1b[1;2;3s",
        b"\x1b[>1r",
        b"\x1b[=1s",
    ] {
        expect(bytes, &[]);
    }
}

#[test]
fn sgr_reuses_the_parameter_parser() {
    use maestro_terminal::{
        color::Rgb,
        sgr::{Attribute as A, Underline},
    };
    expect(
        b"\x1b[m\x1b[0m\x1b[1;3;4:3;38:2::12:34:56;48;5;123;58:5:42m",
        &[
            Action::SetAttribute(A::Unset),
            Action::SetAttribute(A::Unset),
            Action::SetAttribute(A::Bold),
            Action::SetAttribute(A::Italic),
            Action::SetAttribute(A::Underline(Underline::Curly)),
            Action::SetAttribute(A::DirectColorFg(Rgb {
                r: 12,
                g: 34,
                b: 56,
            })),
            Action::SetAttribute(A::Palette256Bg(123)),
            Action::SetAttribute(A::Palette256UnderlineColor(42)),
        ],
    );
    expect(b"\x1b[1:2H", &[]);
    let params = vec!["1"; 25].join(";");
    expect(format!("\x1b[{params}m").as_bytes(), &[]);
    expect(
        b"\x1b[999999m",
        &[Action::SetAttribute(A::Unknown(
            maestro_terminal::sgr::Unknown {
                full: &[65535],
                partial: &[65535],
            },
        ))],
    );
}

#[test]
fn apc_and_dcs_keep_payload_and_exit_order() {
    for (bytes, actions) in payload_cases() {
        for split in 0..=bytes.len() {
            let mut recorder = Expected {
                actions: &actions,
                seen: 0,
            };
            let mut stream = Stream::new(false);
            stream.next_slice(&bytes[..split], &mut recorder);
            stream.next_slice(&bytes[split..], &mut recorder);
            assert_eq!(recorder.seen, actions.len());
        }
    }
}

fn aborted_apc() -> Vec<Action<'static>> {
    vec![
        Action::ApcStart,
        Action::ApcPut(b'G'),
        Action::ApcEnd,
        Action::Print('x'),
    ]
}

fn payload_cases() -> Vec<(&'static [u8], Vec<Action<'static>>)> {
    use maestro_terminal::parser::Dcs;
    vec![
        (
            b"\x1b_G\0\x07\xffX\x1b\\".as_slice(),
            vec![
                Action::ApcStart,
                Action::ApcPut(b'G'),
                Action::ApcPut(0),
                Action::ApcPut(7),
                Action::ApcPut(b'X'),
                Action::ApcEnd,
            ],
        ),
        (b"\x1b_G\x18x", aborted_apc()),
        (b"\x1b_G\x1ax", aborted_apc()),
        (b"\x1b_G\x9cx", aborted_apc()),
        (
            b"\x1b_G\x1bDx",
            vec![
                Action::ApcStart,
                Action::ApcPut(b'G'),
                Action::ApcEnd,
                Action::Index,
                Action::Print('x'),
            ],
        ),
        (
            b"\x1bP1;2$qab\x1b\\x",
            vec![
                Action::DcsHook(Dcs {
                    intermediates: b"$",
                    params: &[1, 2],
                    r#final: b'q',
                }),
                Action::DcsPut(b'a'),
                Action::DcsPut(b'b'),
                Action::DcsUnhook,
                Action::Print('x'),
            ],
        ),
    ]
}

#[test]
fn osc_dispatch_keeps_owned_command_values() {
    use maestro_terminal::{
        kitty::color::{Kind, Osc, Request as K, Special},
        osc::{
            Terminator,
            color::{Operation, Request, Target},
            semantic_prompt::{Action as P, Command},
        },
    };
    let actions = [
        Action::SemanticPrompt(Command {
            action: P::PromptStart,
            options_unvalidated: b"k=r;redraw=1".to_vec(),
        }),
        Action::WindowTitle(b"title".to_vec()),
        Action::WindowTitle(vec![]),
        Action::ClipboardContents {
            kind: b'c',
            data: b"?".to_vec(),
        },
        Action::ReportPwd(b"file:///tmp".to_vec()),
        Action::StartHyperlink {
            id: Some(b"one".to_vec()),
            uri: b"https://example.test".to_vec(),
        },
        Action::EndHyperlink,
        Action::ColorOperation {
            op: Operation::Osc104,
            requests: vec![Request::Reset(Target::Palette(3))],
            terminator: Terminator::Bel,
        },
        Action::KittyColorReport(Osc {
            list: vec![K::Reset(Kind::Special(Special::Cursor))],
            terminator: Terminator::St,
        }),
    ];
    let bytes = b"\x1b]133;P;k=r;redraw=1\x07\x1b]0;title\x07\x1b]2;\x07\x1b]52;c;?\x07\x1b]7;file:///tmp\x07\x1b]8;id=one;https://example.test\x07\x1b]8;;\x07\x1b]104;3\x07\x1b]21;cursor=\x1b\\";
    for split in 0..=bytes.len() {
        let mut recorder = Expected {
            actions: &actions,
            seen: 0,
        };
        let mut stream = Stream::new(true);
        stream.next_slice(&bytes[..split], &mut recorder);
        stream.next_slice(&bytes[split..], &mut recorder);
        assert_eq!(recorder.seen, actions.len());
    }
    expect(b"\x1b]0;bad\xff\x07x", &[Action::Print('x')]);
}

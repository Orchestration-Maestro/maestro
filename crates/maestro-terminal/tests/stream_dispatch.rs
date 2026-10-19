//! Typed actions observed at the byte-stream boundary.

use maestro_terminal::stream::{
    CharsetAction, CursorAction, EditAction, EraseAction, KeyboardAction, ModeAction, OscAction,
    PayloadAction, ProtectionAction, QueryAction, TabAction,
};

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
            Action::Query(QueryAction::Enquiry),
            Action::Bell,
            Action::Cursor(CursorAction::Backspace),
            Action::Tab(TabAction::HorizontalTab(1)),
            Action::Cursor(CursorAction::Linefeed),
            Action::Cursor(CursorAction::Linefeed),
            Action::Cursor(CursorAction::Linefeed),
            Action::Cursor(CursorAction::CarriageReturn),
            Action::Charset(CharsetAction::InvokeCharset {
                bank: maestro_terminal::CharsetActiveSlot::Gl,
                charset: maestro_terminal::CharsetSlot::G1,
                locking: false,
            }),
            Action::Charset(CharsetAction::InvokeCharset {
                bank: maestro_terminal::CharsetActiveSlot::Gl,
                charset: maestro_terminal::CharsetSlot::G0,
                locking: false,
            }),
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
    expect(b"\x1b[\x84", &[Action::Cursor(CursorAction::Index)]);
}

type CountAction = fn(u16) -> Action<'static>;

#[test]
fn count_commands_keep_defaults_and_rejections() {
    let commands: &[(u8, CountAction)] = &[
        (b'A', |value| Action::Cursor(CursorAction::CursorUp(value))),
        (b'k', |value| Action::Cursor(CursorAction::CursorUp(value))),
        (b'B', |value| {
            Action::Cursor(CursorAction::CursorDown(value))
        }),
        (b'C', |value| {
            Action::Cursor(CursorAction::CursorRight(value))
        }),
        (b'D', |value| {
            Action::Cursor(CursorAction::CursorLeft(value))
        }),
        (b'j', |value| {
            Action::Cursor(CursorAction::CursorLeft(value))
        }),
        (b'G', |value| Action::Cursor(CursorAction::CursorCol(value))),
        (b'`', |value| Action::Cursor(CursorAction::CursorCol(value))),
        (b'I', |value| Action::Tab(TabAction::HorizontalTab(value))),
        (b'L', |value| Action::Edit(EditAction::InsertLines(value))),
        (b'M', |value| Action::Edit(EditAction::DeleteLines(value))),
        (b'P', |value| Action::Edit(EditAction::DeleteChars(value))),
        (b'S', |value| Action::Edit(EditAction::ScrollUp(value))),
        (b'T', |value| Action::Edit(EditAction::ScrollDown(value))),
        (b'X', |value| Action::Edit(EditAction::EraseChars(value))),
        (b'Z', |value| {
            Action::Tab(TabAction::HorizontalTabBack(value))
        }),
        (b'a', |value| {
            Action::Cursor(CursorAction::CursorColRelative(value))
        }),
        (b'b', |value| Action::Edit(EditAction::PrintRepeat(value))),
        (b'd', |value| Action::Cursor(CursorAction::CursorRow(value))),
        (b'e', |value| {
            Action::Cursor(CursorAction::CursorRowRelative(value))
        }),
        (b'@', |value| Action::Edit(EditAction::InsertBlanks(value))),
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
    count_commands_with_carriage_return();
}

fn count_commands_with_carriage_return() {
    for (command, action) in [
        (
            b'E',
            (|value| Action::Cursor(CursorAction::CursorDown(value))) as CountAction,
        ),
        (b'F', |value| Action::Cursor(CursorAction::CursorUp(value))),
    ] {
        for (operand, value) in [("", 1), ("0", 0), ("1", 1), ("65535", 65535)] {
            expect(
                format!("\x1b[{operand}{}", char::from(command)).as_bytes(),
                &[action(value), Action::Cursor(CursorAction::CarriageReturn)],
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
                &[Action::Cursor(CursorAction::CursorPos { row, col })],
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
        expect(bytes, &[Action::Tab(TabAction::TabSet)]);
    }
    expect(b"\x1b[2W", &[Action::Tab(TabAction::TabClearCurrent)]);
    expect(b"\x1b[5W", &[Action::Tab(TabAction::TabClearAll)]);
    expect(b"\x1b[?5W", &[Action::Tab(TabAction::TabReset)]);
    expect(
        b"\x1b[g\x1b[0g\x1b[3g",
        &[
            Action::Tab(TabAction::TabClearCurrent),
            Action::Tab(TabAction::TabClearCurrent),
            Action::Tab(TabAction::TabClearAll),
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
    use maestro_terminal::{Charset, CharsetActiveSlot as Bank, CharsetSlot as Slot};
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
                &[Action::Charset(CharsetAction::ConfigureCharset {
                    slot,
                    charset,
                })],
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
            &[Action::Charset(CharsetAction::InvokeCharset {
                bank,
                charset,
                locking,
            })],
        );
        expect(format!("\x1b {code}").as_bytes(), &[]);
    }
    esc_control_actions();
}

fn esc_control_actions() {
    use maestro_terminal::Mode;
    expect(
        b"\x1b7\x1b8\x1b#8\x1bD\x1bE\x1bH\x1bM\x1bV\x1bW\x1bZ\x1bc\x1b=\x1b>\x1b\\",
        &[
            Action::Cursor(CursorAction::SaveCursor),
            Action::Cursor(CursorAction::RestoreCursor),
            Action::Edit(EditAction::Decaln),
            Action::Cursor(CursorAction::Index),
            Action::Cursor(CursorAction::NextLine),
            Action::Tab(TabAction::TabSet),
            Action::Cursor(CursorAction::ReverseIndex),
            Action::Protection(ProtectionAction::ProtectedModeIso),
            Action::Protection(ProtectionAction::ProtectedModeOff),
            Action::Query(QueryAction::DeviceAttributes(
                maestro_terminal::device_attributes::Req::Primary,
            )),
            Action::FullReset,
            Action::Mode(ModeAction::SetMode(Mode::KeypadKeys)),
            Action::Mode(ModeAction::ResetMode(Mode::KeypadKeys)),
        ],
    );
    for code in ['7', '8', 'D', 'E', 'H', 'M', 'V', 'W', 'Z', 'c', '=', '>'] {
        expect(format!("\x1b {code}").as_bytes(), &[]);
    }
    expect(b"\x1bQ\x1b-B", &[]);
}

#[test]
fn erase_dispatch_keeps_protection_and_admitted_modes() {
    type EraseConstructor = fn(bool) -> Action<'static>;
    let display: &[(u16, EraseConstructor)] = &[
        (0, |value| {
            Action::Erase(EraseAction::EraseDisplayBelow(value))
        }),
        (1, |value| {
            Action::Erase(EraseAction::EraseDisplayAbove(value))
        }),
        (2, |value| {
            Action::Erase(EraseAction::EraseDisplayComplete(value))
        }),
        (3, |value| {
            Action::Erase(EraseAction::EraseDisplayScrollback(value))
        }),
        (22, |value| {
            Action::Erase(EraseAction::EraseDisplayScrollComplete(value))
        }),
    ];
    let line: &[(u16, EraseConstructor)] = &[
        (0, |value| Action::Erase(EraseAction::EraseLineRight(value))),
        (1, |value| Action::Erase(EraseAction::EraseLineLeft(value))),
        (2, |value| {
            Action::Erase(EraseAction::EraseLineComplete(value))
        }),
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
            Action::Mode(ModeAction::SetMode(Mode::Insert)), Action::Mode(ModeAction::SetMode(Mode::Insert)),
            Action::Mode(ModeAction::SetMode(Mode::Linefeed)), Action::Mode(ModeAction::ResetMode(Mode::Insert)),
            Action::Mode(ModeAction::ResetMode(Mode::Linefeed)), Action::Mode(ModeAction::SetMode(Mode::Wraparound)),
            Action::Mode(ModeAction::SetMode(Mode::Origin)), Action::Mode(ModeAction::SetMode(Mode::Wraparound)),
            Action::Mode(ModeAction::ResetMode(Mode::Origin)), Action::Mode(ModeAction::ResetMode(Mode::Wraparound)),
            Action::Mode(ModeAction::SaveMode(Mode::Wraparound)), Action::Mode(ModeAction::SaveMode(Mode::Origin)),
            Action::Mode(ModeAction::SaveMode(Mode::Wraparound)), Action::Mode(ModeAction::RestoreMode(Mode::Origin)),
            Action::Mode(ModeAction::RestoreMode(Mode::Wraparound)),
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
            Action::Query(QueryAction::RequestMode(Mode::Insert)),
            Action::Query(QueryAction::RequestModeUnknown(ModeTag {
                value: 999,
                ansi: true,
            })),
            Action::Query(QueryAction::RequestMode(Mode::Wraparound)),
            Action::Query(QueryAction::RequestModeUnknown(ModeTag {
                value: 999,
                ansi: false,
            })),
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
            &[Action::Keyboard(KeyboardAction::ModifyKeyFormat(format))],
        );
    }
    for params in ["", "4", "4;2;9", "65535"] {
        expect(
            format!("\x1b[>{params}n").as_bytes(),
            &[Action::Keyboard(KeyboardAction::ModifyKeyFormat(
                F::OtherKeysNumericExcept,
            ))],
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
            &[Action::Keyboard(KeyboardAction::KittyKeyboardPush(flags))],
        );
        for (op, action) in [
            (1, Action::Keyboard(KeyboardAction::KittyKeyboardSet(flags))),
            (
                2,
                Action::Keyboard(KeyboardAction::KittyKeyboardSetOr(flags)),
            ),
            (
                3,
                Action::Keyboard(KeyboardAction::KittyKeyboardSetNot(flags)),
            ),
        ] {
            expect(format!("\x1b[={value};{op};9u").as_bytes(), &[action]);
        }
    }
    keyboard_defaults_and_rejections();
}

fn keyboard_defaults_and_rejections() {
    use maestro_terminal::kitty::key::Flags;
    for params in ["", "32;7", "65535;2"] {
        expect(
            format!("\x1b[>{params}u").as_bytes(),
            &[Action::Keyboard(KeyboardAction::KittyKeyboardPush(
                Flags::DISABLED,
            ))],
        );
        expect(
            format!("\x1b[<{params}u").as_bytes(),
            &[Action::Keyboard(KeyboardAction::KittyKeyboardPop(1))],
        );
    }
    for params in ["", "0", "65535;2"] {
        expect(
            format!("\x1b[?{params}u").as_bytes(),
            &[Action::Query(QueryAction::KittyKeyboardQuery)],
        );
        expect(
            format!("\x1b[{params}u").as_bytes(),
            &[Action::Cursor(CursorAction::RestoreCursor)],
        );
    }
    for value in [0, 1, 65535] {
        expect(
            format!("\x1b[<{value}u").as_bytes(),
            &[Action::Keyboard(KeyboardAction::KittyKeyboardPop(value))],
        );
    }
    expect(
        b"\x1b[=u",
        &[Action::Keyboard(KeyboardAction::KittyKeyboardSet(
            Flags::DISABLED,
        ))],
    );
    expect(
        b"\x1b[=3u",
        &[Action::Keyboard(KeyboardAction::KittyKeyboardSet(
            Flags::from_int(3).unwrap(),
        ))],
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
                &[Action::Query(QueryAction::DeviceAttributes(req))],
            );
        }
        expect(
            format!("\x1b[>{params}q").as_bytes(),
            &[Action::Query(QueryAction::Xtversion)],
        );
    }
    expect(
        b"\x1b[5n\x1b[6n\x1b[?996n",
        &[
            Action::Query(QueryAction::DeviceStatus(R::OperatingStatus)),
            Action::Query(QueryAction::DeviceStatus(R::CursorPosition)),
            Action::Query(QueryAction::DeviceStatus(R::ColorScheme)),
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
            &[Action::Query(QueryAction::SizeReport(style))],
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
            Action::Protection(ProtectionAction::ProtectedModeOff),
            Action::Protection(ProtectionAction::ProtectedModeOff),
            Action::Protection(ProtectionAction::ProtectedModeDec),
            Action::Protection(ProtectionAction::ProtectedModeOff),
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
        (
            22,
            (|value| Action::Query(QueryAction::TitlePush(value))) as CountAction,
        ),
        (23, |value| Action::Query(QueryAction::TitlePop(value))),
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
    margin_terminal_consumers();
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
        Action::Payload(PayloadAction::ApcStart),
        Action::Payload(PayloadAction::ApcPut(b'G')),
        Action::Payload(PayloadAction::ApcEnd),
        Action::Print('x'),
    ]
}

fn payload_cases() -> Vec<(&'static [u8], Vec<Action<'static>>)> {
    use maestro_terminal::parser::Dcs;
    vec![
        (
            b"\x1b_G\0\x07\xffX\x1b\\".as_slice(),
            vec![
                Action::Payload(PayloadAction::ApcStart),
                Action::Payload(PayloadAction::ApcPut(b'G')),
                Action::Payload(PayloadAction::ApcPut(0)),
                Action::Payload(PayloadAction::ApcPut(7)),
                Action::Payload(PayloadAction::ApcPut(b'X')),
                Action::Payload(PayloadAction::ApcEnd),
            ],
        ),
        (b"\x1b_G\x18x", aborted_apc()),
        (b"\x1b_G\x1ax", aborted_apc()),
        (b"\x1b_G\x9cx", aborted_apc()),
        (
            b"\x1b_G\x1bDx",
            vec![
                Action::Payload(PayloadAction::ApcStart),
                Action::Payload(PayloadAction::ApcPut(b'G')),
                Action::Payload(PayloadAction::ApcEnd),
                Action::Cursor(CursorAction::Index),
                Action::Print('x'),
            ],
        ),
        (
            b"\x1bP1;2$qab\x1b\\x",
            vec![
                Action::Payload(PayloadAction::DcsHook(Dcs {
                    intermediates: b"$",
                    params: &[1, 2],
                    r#final: b'q',
                })),
                Action::Payload(PayloadAction::DcsPut(b'a')),
                Action::Payload(PayloadAction::DcsPut(b'b')),
                Action::Payload(PayloadAction::DcsUnhook),
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
        Action::Osc(OscAction::SemanticPrompt(Command {
            action: P::PromptStart,
            options_unvalidated: b"k=r;redraw=1".to_vec(),
        })),
        Action::Osc(OscAction::WindowTitle(b"title".to_vec())),
        Action::Osc(OscAction::WindowTitle(vec![])),
        Action::Osc(OscAction::ClipboardContents {
            kind: b'c',
            data: b"?".to_vec(),
        }),
        Action::Osc(OscAction::ReportPwd(b"file:///tmp".to_vec())),
        Action::Osc(OscAction::StartHyperlink {
            id: Some(b"one".to_vec()),
            uri: b"https://example.test".to_vec(),
        }),
        Action::Osc(OscAction::EndHyperlink),
        Action::Osc(OscAction::ColorOperation {
            op: Operation::Osc104,
            requests: vec![Request::Reset(Target::Palette(3))],
            terminator: Terminator::Bel,
        }),
        Action::Osc(OscAction::KittyColorReport(Osc {
            list: vec![K::Reset(Kind::Special(Special::Cursor))],
            terminator: Terminator::St,
        })),
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

fn margin_terminal_consumers() {
    let mut t = maestro_terminal::Terminal::new(12, 10, 0).unwrap();
    t.write(b"\x1b[2;3H\x1b[s\x1b[H\x1b[u");
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (2, 1)
    );
    t.write(b"\x1b[?69h\x1b[4;9s");
    assert_eq!((t.scrolling_region.left, t.scrolling_region.right), (3, 8));
    t.write(b"\x1b[s");
    assert_eq!((t.scrolling_region.left, t.scrolling_region.right), (0, 11));
}

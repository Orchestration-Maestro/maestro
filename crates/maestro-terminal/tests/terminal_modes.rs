//! Terminal mode numbers, retained values and report bytes.

use maestro_terminal::Mode;
use maestro_terminal::modes::{ModeState, ModeTag, Report, ReportState};

#[test]
fn mode_state_restores_values_and_reset_defaults() {
    let mut state = ModeState::default();
    let enabled = [
        Mode::SendReceiveMode,
        Mode::Wraparound,
        Mode::CursorVisible,
        Mode::MouseAlternateScroll,
        Mode::IgnoreKeypadWithNumlock,
        Mode::AltEscPrefix,
    ];
    for ansi in [false, true] {
        for number in 0..=u16::MAX {
            if let Some(mode) = Mode::from_number(number, ansi) {
                assert_eq!(state.get(mode), enabled.contains(&mode));
                assert_eq!(mode.tag().value, number);
                assert_eq!(mode.tag().ansi, ansi);
                let initial = state.get(mode);
                state.set(mode, !initial);
                assert_eq!(state.get(mode), !initial);
                state.save(mode);
                state.set(mode, initial);
                assert_eq!(state.restore(mode), !initial);
                assert_eq!(state.get(mode), !initial);
                state.save(mode);
                state.set(mode, initial);
                state.save(mode);
                state.set(mode, !initial);
                assert_eq!(state.restore(mode), initial);
            }
        }
    }
    state.set(Mode::CursorKeys, true);
    state.save(Mode::CursorKeys);
    state.reset();
    assert!(!state.get(Mode::CursorKeys));
    assert!(!state.restore(Mode::CursorKeys));
    assert!(state.get(Mode::Wraparound));
    state.set(Mode::Wraparound, false);
    state.save(Mode::Wraparound);
    state.reset();
    assert!(state.restore(Mode::Wraparound));
}

#[test]
fn mode_numbers_keep_ansi_and_private_namespaces() {
    let ansi = [
        (2, Mode::DisableKeyboard),
        (4, Mode::Insert),
        (12, Mode::SendReceiveMode),
        (20, Mode::Linefeed),
    ];
    let private = [
        (1, Mode::CursorKeys),
        (3, Mode::Column132),
        (4, Mode::SlowScroll),
        (5, Mode::ReverseColors),
        (6, Mode::Origin),
        (7, Mode::Wraparound),
        (8, Mode::Autorepeat),
        (9, Mode::MouseEventX10),
        (12, Mode::CursorBlinking),
        (25, Mode::CursorVisible),
        (40, Mode::EnableMode3),
        (45, Mode::ReverseWrap),
        (47, Mode::AltScreenLegacy),
        (66, Mode::KeypadKeys),
        (67, Mode::BackarrowKeyMode),
        (69, Mode::EnableLeftAndRightMargin),
        (1000, Mode::MouseEventNormal),
        (1002, Mode::MouseEventButton),
        (1003, Mode::MouseEventAny),
        (1004, Mode::FocusEvent),
        (1005, Mode::MouseFormatUtf8),
        (1006, Mode::MouseFormatSgr),
        (1007, Mode::MouseAlternateScroll),
        (1015, Mode::MouseFormatUrxvt),
        (1016, Mode::MouseFormatSgrPixels),
        (1035, Mode::IgnoreKeypadWithNumlock),
        (1036, Mode::AltEscPrefix),
        (1039, Mode::AltSendsEscape),
        (1045, Mode::ReverseWrapExtended),
        (1047, Mode::AltScreen),
        (1048, Mode::SaveCursor),
        (1049, Mode::AltScreenSaveCursorClearEnter),
        (2004, Mode::BracketedPaste),
        (2026, Mode::SynchronizedOutput),
        (2027, Mode::GraphemeCluster),
        (2031, Mode::ReportColorScheme),
        (2048, Mode::InBandSizeReports),
    ];
    for (namespace, entries) in [(true, ansi.as_slice()), (false, private.as_slice())] {
        for number in 0..=u16::MAX {
            let expected = entries
                .iter()
                .find(|entry| entry.0 == number)
                .map(|entry| entry.1);
            assert_eq!(
                Mode::from_number(number, namespace),
                expected,
                "number {number}, ANSI {namespace}"
            );
        }
    }
}

#[test]
fn mode_report_known_dec_mode() {
    let mut state = ModeState::default();
    let tag = ModeTag {
        value: 1,
        ansi: false,
    };
    let report = state.get_report(tag);
    assert_eq!(report.state, ReportState::Reset);
    assert_eq!(report.tag, tag);
    state.set(Mode::CursorKeys, true);
    assert_eq!(state.get_report(tag).state, ReportState::Set);
}

#[test]
fn mode_report_dec_mode_set() {
    let report = Report {
        tag: ModeTag {
            value: 1,
            ansi: false,
        },
        state: ReportState::Set,
    };
    let mut bytes = Vec::new();
    report.encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[?1;1$y");
}

#[test]
fn mode_report_known_ansi_mode() {
    let mut state = ModeState::default();
    state.set(Mode::Insert, true);
    let tag = ModeTag {
        value: 4,
        ansi: true,
    };
    let report = state.get_report(tag);
    assert_eq!(report.state, ReportState::Set);
    assert_eq!(report.tag, tag);
}

#[test]
fn mode_report_unknown_mode() {
    let state = ModeState::default();
    assert_eq!(
        state
            .get_report(ModeTag {
                value: 9999,
                ansi: false
            })
            .state,
        ReportState::NotRecognized
    );
}

#[test]
fn mode_report_dec_mode_reset() {
    let report = Report {
        tag: ModeTag {
            value: 1,
            ansi: false,
        },
        state: ReportState::Reset,
    };
    let mut bytes = Vec::new();
    report.encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[?1;2$y");
}

#[test]
fn mode_report_ansi_mode() {
    let report = Report {
        tag: ModeTag {
            value: 4,
            ansi: true,
        },
        state: ReportState::Set,
    };
    let mut bytes = Vec::new();
    report.encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[4;1$y");
}

#[test]
fn mode_report_not_recognized() {
    let report = Report {
        tag: ModeTag {
            value: 9999,
            ansi: false,
        },
        state: ReportState::NotRecognized,
    };
    let mut bytes = Vec::new();
    report.encode(&mut bytes).unwrap();
    assert_eq!(bytes, b"\x1b[?9999;0$y");
}

#[test]
fn terminal_mode_reports_cover_all_response_states() {
    let cases = [
        (
            ReportState::NotRecognized,
            b"\x1b[?32767;0$y",
            b"\x1b[32767;0$y",
        ),
        (ReportState::Set, b"\x1b[?32767;1$y", b"\x1b[32767;1$y"),
        (ReportState::Reset, b"\x1b[?32767;2$y", b"\x1b[32767;2$y"),
        (
            ReportState::PermanentlySet,
            b"\x1b[?32767;3$y",
            b"\x1b[32767;3$y",
        ),
        (
            ReportState::PermanentlyReset,
            b"\x1b[?32767;4$y",
            b"\x1b[32767;4$y",
        ),
    ];
    for (state, private, ansi) in cases {
        for (namespace, expected) in [(false, private.as_slice()), (true, ansi.as_slice())] {
            let report = Report {
                tag: ModeTag {
                    value: 32767,
                    ansi: namespace,
                },
                state,
            };
            let mut bytes = Vec::new();
            report.encode(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(
                report
                    .encode(&mut [0_u8; 1].as_mut_slice())
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::WriteZero
            );
        }
    }
    assert_eq!(
        ModeState::default()
            .get_report(ModeTag {
                value: 9999,
                ansi: true
            })
            .state,
        ReportState::NotRecognized
    );
}

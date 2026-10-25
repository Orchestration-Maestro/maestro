//! Constant VT transitions through the terminal's public table.

use maestro_terminal::{
    parse_table::TABLE,
    parser::{State, TransitionAction},
};
use serde::Deserialize;

/// A byte/state pair and its recorded transition.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Fixture family.
    kind: String,
    /// Input byte.
    byte: usize,
    /// Prior state index.
    state: usize,
    /// Resulting state and transition action.
    value: Expected,
}

/// The complete output of one table lookup.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Destination state name.
    state: String,
    /// Transition action name.
    action: String,
}

/// States in their public numeric order.
const STATES: [(State, &str); 14] = [
    (State::Ground, "ground"),
    (State::Escape, "escape"),
    (State::EscapeIntermediate, "escape_intermediate"),
    (State::CsiEntry, "csi_entry"),
    (State::CsiIntermediate, "csi_intermediate"),
    (State::CsiParam, "csi_param"),
    (State::CsiIgnore, "csi_ignore"),
    (State::DcsEntry, "dcs_entry"),
    (State::DcsParam, "dcs_param"),
    (State::DcsIntermediate, "dcs_intermediate"),
    (State::DcsPassthrough, "dcs_passthrough"),
    (State::DcsIgnore, "dcs_ignore"),
    (State::OscString, "osc_string"),
    (State::SosPmApcString, "sos_pm_apc_string"),
];

/// Transition actions in their public numeric order.
const ACTIONS: [(TransitionAction, &str); 11] = [
    (TransitionAction::None, "none"),
    (TransitionAction::Ignore, "ignore"),
    (TransitionAction::Print, "print"),
    (TransitionAction::Execute, "execute"),
    (TransitionAction::Collect, "collect"),
    (TransitionAction::Param, "param"),
    (TransitionAction::EscDispatch, "esc_dispatch"),
    (TransitionAction::CsiDispatch, "csi_dispatch"),
    (TransitionAction::Put, "put"),
    (TransitionAction::OscPut, "osc_put"),
    (TransitionAction::ApcPut, "apc_put"),
];

#[test]
fn vt_transition_values_match_every_byte_state() {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/vt-transitions.json")).unwrap();
    assert_eq!(rows.len(), 256 * STATES.len());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.kind, "transition");
        assert_eq!((row.byte, row.state), (index / 14, index % 14));
        let prior = STATES[row.state].0;
        assert_eq!(prior as usize, row.state);
        let observed = TABLE[row.byte][prior as usize];
        let state = STATES
            .iter()
            .find(|(_, name)| *name == row.value.state)
            .unwrap()
            .0;
        let action = ACTIONS
            .iter()
            .find(|(_, name)| *name == row.value.action)
            .unwrap()
            .0;
        assert_eq!(observed.state, state, "state at case {index}");
        assert_eq!(observed.action, action, "action at case {index}");
    }
}

#[test]
fn numeric_saturation_survives_long_fields() {
    use maestro_terminal::parser::{Action, Parser};
    for digits in [255, 256, 257, 512] {
        for dcs in [false, true] {
            let mut parser = Parser::init();
            let prefix: &[u8] = if dcs { b"\x1bP" } else { b"\x1b[" };
            for byte in prefix
                .iter()
                .copied()
                .chain(std::iter::repeat_n(b'9', digits))
            {
                let _ = parser.next(byte);
            }
            let actions = parser.next(if dcs { b'q' } else { b'm' });
            let params = match &actions[if dcs { 2 } else { 1 }] {
                Some(Action::CsiDispatch(v)) => &*v.params,
                Some(Action::DcsHook(v)) => v.params,
                _ => panic!("missing parameter dispatch"),
            };
            assert_eq!(params, [65535]);
            parser.clear();
            for byte in prefix
                .iter()
                .copied()
                .chain(std::iter::repeat_n(b'9', digits))
                .chain(b";3".iter().copied())
            {
                let _ = parser.next(byte);
            }
            let actions = parser.next(if dcs { b'q' } else { b'm' });
            let params = match &actions[if dcs { 2 } else { 1 }] {
                Some(Action::CsiDispatch(v)) => &*v.params,
                Some(Action::DcsHook(v)) => v.params,
                _ => panic!("missing separated numeric dispatch"),
            };
            assert_eq!(params, [65535, 3]);
        }
    }
}

use maestro_terminal::parser::{Action, Parser};

/// Assert the writable CSI view in its exact transition slot.
fn assert_csi(input: &[u8], final_byte: u8, intermediates: &[u8], params: &[u16], mask: u32) {
    let mut parser = Parser::init();
    for byte in &input[..input.len() - 1] {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let actions = parser.next(*input.last().unwrap());
    assert!(actions[0].is_none() && actions[2].is_none());
    let Some(Action::CsiDispatch(v)) = &actions[1] else {
        panic!("missing CSI dispatch");
    };
    assert_eq!(v.r#final, final_byte);
    assert_eq!(v.intermediates, intermediates);
    assert_eq!(v.params, params);
    assert_eq!(v.params_sep.0, mask);
    assert_eq!(parser.state, State::Ground);
}

/// Build a parameter-list input with no hidden trailing delimiter.
fn parameter_input(prefix: &str, count: usize, suffix: &str) -> Vec<u8> {
    format!(
        "{prefix}{}{suffix}",
        (1..=count)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(";")
    )
    .into_bytes()
}

#[test]
fn decode_csi_esc_h() {
    assert_csi(b"\x1b[H", b'H', b"", &[], 0);
}

#[test]
fn decode_csi_esc_1_4_h() {
    assert_csi(b"\x1b[1;4H", b'H', b"", &[1, 4], 0);
}

#[test]
fn decode_csi_sgr_esc_38_2_m() {
    assert_csi(b"\x1b[38:2m", b'm', b"", &[38, 2], 1);
}

#[test]
fn decode_csi_sgr_colon_followed_by_semicolon() {
    assert_csi(b"\x1b[38:5:1;48:5:0m", b'm', b"", &[38, 5, 1, 48, 5, 0], 27);
}

#[test]
fn decode_csi_sgr_mixed_colon_and_semicolon() {
    assert_csi(b"\x1b[1:2;3:4m", b'm', b"", &[1, 2, 3, 4], 5);
}

#[test]
fn decode_csi_sgr_esc_48_2_m() {
    assert_csi(
        b"\x1b[48:2:240:143:104m",
        b'm',
        b"",
        &[48, 2, 240, 143, 104],
        15,
    );
}

#[test]
fn decode_csi_sgr_esc_4_3m_colon() {
    assert_csi(b"\x1b[4:3m", b'm', b"", &[4, 3], 1);
}

#[test]
fn decode_csi_sgr_with_many_blank_and_colon() {
    assert_csi(
        b"\x1b[58:2::240:143:104m",
        b'm',
        b"",
        &[58, 2, 0, 240, 143, 104],
        31,
    );
}

#[test]
fn decode_csi_sgr_mixed_colon_and_semicolon_with_blank() {
    assert_csi(
        b"\x1b[;4:3;38;2;175;175;215;58:2::190:80:70m",
        b'm',
        b"",
        &[0, 4, 3, 38, 2, 175, 175, 215, 58, 2, 0, 190, 80, 70],
        7938,
    );
}

#[test]
fn decode_csi_sgr_mixed_colon_and_semicolon_setting_underline_bg_fg() {
    assert_csi(
        b"\x1b[4:3;38;2;51;51;51;48;2;170;170;170;58;2;255;97;136m",
        b'm',
        b"",
        &[
            4, 3, 38, 2, 51, 51, 51, 48, 2, 170, 170, 170, 58, 2, 255, 97, 136,
        ],
        1,
    );
}

#[test]
fn decode_csi_request_mode_decrqm() {
    assert_csi(b"\x1b[?2026$p", b'p', b"?$", &[2026], 0);
}

#[test]
fn decode_csi_change_cursor() {
    assert_csi(b"\x1b[3 q", b'q', b" ", &[3], 0);
}

#[test]
fn decode_parser_domain() {
    let mut parser = Parser::init();
    for byte in [0x98, 0x9e, 0x9f] {
        assert_eq!(parser.next(byte), [None, None, Some(Action::ApcStart)]);
        assert_eq!(parser.state, State::SosPmApcString);
        assert_eq!(parser.next(0x9c), [Some(Action::ApcEnd), None, None]);
        assert_eq!(parser.state, State::Ground);
    }
    assert_eq!(parser.next(b'a'), [None, Some(Action::Print('a')), None]);
    assert_eq!(parser.next(0x19), [None, Some(Action::Execute(0x19)), None]);
}

#[test]
fn decode_esc_esc_b() {
    let mut parser = Parser::init();
    assert_eq!(parser.next(27), [None, None, None]);
    assert_eq!(parser.next(b'('), [None, None, None]);
    let actions = parser.next(b'B');
    assert!(actions[0].is_none() && actions[2].is_none());
    let Some(Action::EscDispatch(v)) = &actions[1] else {
        panic!("missing ESC dispatch");
    };
    assert_eq!(v.intermediates, b"(");
    assert_eq!(v.r#final, b'B');
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_csi_colon_for_non_m_final() {
    let mut parser = Parser::init();
    for byte in b"\x1b[1:2H" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::Ground);
    assert_csi(b"\x1b[1;2H", b'H', b"", &[1, 2], 0);
}

#[test]
fn decode_osc_112_incomplete_sequence() {
    let mut parser = Parser::init();
    for byte in b"\x1b]112;\x07" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_osc_104_empty() {
    let mut parser = Parser::init();
    for byte in b"\x1b]104\x07" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_osc_change_window_title() {
    let mut parser = Parser::init();
    for byte in b"\x1b]0;window" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    assert_eq!(
        parser.next(7),
        [
            Some(Action::OscDispatch(
                maestro_terminal::osc::Command::ChangeWindowTitle(b"window".to_vec())
            )),
            None,
            None
        ]
    );
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_osc_change_window_title_end_in_esc() {
    let mut parser = Parser::init();
    for byte in b"\x1b]0;window" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    assert_eq!(
        parser.next(27),
        [
            Some(Action::OscDispatch(
                maestro_terminal::osc::Command::ChangeWindowTitle(b"window".to_vec())
            )),
            None,
            None
        ]
    );
    assert_eq!(parser.state, State::Escape);
}

#[test]
fn decode_csi_too_many_params() {
    let mut parser = Parser::init();
    for byte in parameter_input("\x1b[", 100, "m") {
        assert!(parser.next(byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_csi_sgr_beyond_our_max_drops_it() {
    let mut parser = Parser::init();
    for byte in parameter_input("\x1b[", 25, "m") {
        assert!(parser.next(byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_csi_sgr_with_up_to_our_max_parameters() {
    for count in 1..=24 {
        let expected: Vec<u16> = (1..=count).collect();
        assert_csi(
            &parameter_input("\x1b[", usize::from(count), "m"),
            b'm',
            b"",
            &expected,
            0,
        );
    }
}

/// Assert a DCS hook before payload delivery and its complete unhook framing.
fn assert_dcs(prefix: &[u8], intermediates: &[u8], params: &[u16]) {
    let mut parser = Parser::init();
    for byte in prefix {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let actions = parser.next(b'q');
    assert!(actions[0].is_none() && actions[1].is_none());
    let Some(Action::DcsHook(v)) = &actions[2] else {
        panic!("missing DCS hook");
    };
    let v: &maestro_terminal::Dcs<'_> = v;
    assert_eq!(v.intermediates, intermediates);
    assert_eq!(v.params, params);
    assert_eq!(v.r#final, b'q');
    assert_eq!(parser.next(b'x'), [None, Some(Action::DcsPut(b'x')), None]);
    assert_eq!(parser.next(0x9c), [Some(Action::DcsUnhook), None, None]);
    assert_eq!(parser.state, State::Ground);
}

#[test]
fn decode_dcs_xtgettcap() {
    assert_dcs(b"\x1bP+", b"+", &[]);
}

#[test]
fn decode_dcs_params() {
    assert_dcs(b"\x1bP1000", b"", &[1000]);
}

#[test]
fn decode_dcs_too_many_params() {
    let mut parser = Parser::init();
    for byte in parameter_input("\x1bP", 25, "q") {
        assert!(parser.next(byte).iter().all(Option::is_none));
    }
    assert_eq!(parser.state, State::DcsPassthrough);
    assert_eq!(parser.next(b'x'), [None, Some(Action::DcsPut(b'x')), None]);
    assert_eq!(parser.next(0x9c), [Some(Action::DcsUnhook), None, None]);
}

#[test]
fn vt_parameter_and_intermediate_boundaries() {
    assert_csi(b"\x1b[m", b'm', b"", &[], 0);
    assert_csi(b"\x1b[0m", b'm', b"", &[0], 0);
    assert_csi(b"\x1b[;:;:m", b'm', b"", &[0, 0, 0, 0], 10);
    assert_csi(b"\x1b[9999999999m", b'm', b"", &[65535], 0);
    for count in [23, 24] {
        let expected: Vec<u16> = (1..=count).collect();
        assert_csi(
            &parameter_input("\x1b[", usize::from(count), "m"),
            b'm',
            b"",
            &expected,
            0,
        );
    }
    let mut parser = Parser::init();
    for byte in parameter_input("\x1b[", 24, ";m") {
        assert!(parser.next(byte).iter().all(Option::is_none));
    }
    for byte in b"\x1b(((((" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let actions = parser.next(b'B');
    let Some(Action::EscDispatch(v)) = &actions[1] else {
        panic!("missing bounded intermediates");
    };
    assert_eq!(v.intermediates, b"((((");
    assert_eq!(v.r#final, b'B');
    parser.clear();
    for byte in b"\x1b[3" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let actions = parser.next(b'm');
    let Some(Action::CsiDispatch(v)) = &actions[1] else {
        panic!("missing reused parser");
    };
    assert_eq!(v.params, [3]);
}

#[test]
fn vt_special_reset_reaches_colour_command() {
    use maestro_terminal::osc::{
        Command, Terminator,
        color::{Operation, Request},
    };
    for alloc in [false, true] {
        for (input, op, requests) in [
            (
                b"105;".as_slice(),
                Operation::Osc105,
                vec![Request::ResetSpecial],
            ),
            (b"104;", Operation::Osc104, vec![Request::ResetPalette]),
            (
                b"112;",
                Operation::Osc112,
                vec![Request::Reset(
                    maestro_terminal::osc::color::Target::Dynamic(
                        maestro_terminal::color::Dynamic::Cursor,
                    ),
                )],
            ),
        ] {
            let mut parser = maestro_terminal::Parser::init();
            parser.osc_parser.alloc = alloc;
            for byte in b"\x1b]".iter().chain(input) {
                assert!(parser.next(*byte).iter().all(Option::is_none));
            }
            let expected = if alloc {
                Some(Action::OscDispatch(Command::ColorOperation {
                    op,
                    requests,
                    terminator: Terminator::Bel,
                }))
            } else {
                None
            };
            assert_eq!(parser.next(7), [expected, None, None]);
        }
    }
}

/// Complete input, ordered emitted events and final state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VtCase {
    /// Input bytes, including framing and cancellation.
    input: Vec<u8>,
    /// Every emitted event, in order.
    events: Vec<VtEvent>,
    /// Final decoder state.
    state: String,
}

/// An emitted action at its exact byte and action slot.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VtEvent {
    /// Source byte offset.
    index: usize,
    /// Exit/transition/entry slot.
    slot: usize,
    /// State after consuming the byte.
    state: String,
    /// Action variant.
    tag: String,
    /// Full action payload, absent for void variants.
    value: Option<serde_json::Value>,
}

#[test]
fn vt_actions_keep_exit_transition_entry_order() {
    use super::support::parsing::{action, wire_name};
    use serde_json::{Value, json};
    let cases: Vec<VtCase> = serde_json::from_str(include_str!("fixtures/vt-events.json")).unwrap();
    for (case_index, case) in cases.into_iter().enumerate() {
        let mut parser = Parser::init();
        let mut actual = Vec::new();
        for (index, byte) in case.input.into_iter().enumerate() {
            let events: Vec<_> = parser
                .next(byte)
                .into_iter()
                .enumerate()
                .filter_map(|(slot, a)| a.map(|a| (slot, action(a))))
                .collect();
            for (slot, (tag, value)) in events {
                actual.push(json!({"index":index,"slot":slot,"state":wire_name(parser.state),"tag":tag,"value":value}));
            }
        }
        let expected: Vec<_> = case.events.into_iter().map(|v| json!({"index":v.index,"slot":v.slot,"state":v.state,"tag":v.tag,"value":v.value.unwrap_or(Value::Null)})).collect();
        assert_eq!(actual, expected, "VT case {case_index}");
        assert_eq!(wire_name(parser.state), case.state, "VT state {case_index}");
    }
    assert_mutable_views_reach_sgr();
}

/// SDK edits affect later consumers of the actual borrowed view.
fn assert_mutable_views_reach_sgr() {
    use maestro_terminal::sgr::{self, Attribute};
    let mut parser = maestro_terminal::Parser::init();
    for byte in b"\x1b[?1" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let mut actions = parser.next(b'm');
    let Some(Action::CsiDispatch(v)) = &mut actions[1] else {
        panic!("missing writable CSI");
    };
    let v: &mut maestro_terminal::Csi<'_> = v;
    v.params[0] = 3;
    v.intermediates[0] = b'>';
    assert_eq!(v.intermediates, b">");
    assert_eq!(
        sgr::Parser {
            params: v.params,
            params_sep: v.params_sep,
            idx: 0
        }
        .collect::<Vec<_>>(),
        [Attribute::Italic]
    );
    for byte in b"\x1b(" {
        assert!(parser.next(*byte).iter().all(Option::is_none));
    }
    let mut actions = parser.next(b'B');
    let Some(Action::EscDispatch(v)) = &mut actions[1] else {
        panic!("missing writable ESC");
    };
    v.intermediates[0] = b')';
    assert_eq!(v.intermediates, b")");
    assert_eq!(v.r#final, b'B');
}

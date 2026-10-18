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

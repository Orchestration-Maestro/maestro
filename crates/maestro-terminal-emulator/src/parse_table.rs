//! Constant byte/state transitions for VT control sequences.

use crate::parser::{State, TransitionAction};
use State::{
    CsiEntry, CsiIgnore, CsiIntermediate, CsiParam, DcsEntry, DcsIgnore, DcsIntermediate, DcsParam,
    DcsPassthrough, Escape, EscapeIntermediate, Ground, OscString, SosPmApcString,
};
use TransitionAction::{
    ApcPut, Collect, CsiDispatch, EscDispatch, Execute, Ignore, None, OscPut, Param, Print, Put,
};

/// The destination and action for one byte in one state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    /// State after processing the byte.
    pub state: State,
    /// Action taken on the transition.
    pub action: TransitionAction,
}

/// Byte-indexed transitions, with states in declaration order.
pub type Table = [[Transition; 14]; 256];

/// The complete VT transition table, indexed by byte and then state.
pub const TABLE: Table = gen_table();

/// Builds the fixed byte/state lookup at compile time.
const fn gen_table() -> Table {
    let states = [
        Ground,
        Escape,
        EscapeIntermediate,
        CsiEntry,
        CsiIntermediate,
        CsiParam,
        CsiIgnore,
        DcsEntry,
        DcsParam,
        DcsIntermediate,
        DcsPassthrough,
        DcsIgnore,
        OscString,
        SosPmApcString,
    ];
    let mut table = [[Transition {
        state: Ground,
        action: None,
    }; 14]; 256];
    let mut byte = 0u16;
    while byte < 256 {
        let mut index = 0;
        while index < states.len() {
            let (state, action) = transition(byte, states[index]);
            table[byte as usize][index] = Transition { state, action };
            index += 1;
        }
        byte += 1;
    }
    table
}

/// Resolves global controls before the state's printable-byte rules.
const fn transition(byte: u16, state: State) -> (State, TransitionAction) {
    // OSC captures high bytes verbatim, including C1 control values.
    if matches!(state, OscString) && byte >= 0x20 {
        return (state, OscPut);
    }
    match byte {
        0x18 | 0x1a | 0x80..=0x8f | 0x91..=0x97 | 0x99..=0x9a => return (Ground, Execute),
        0x9c => return (Ground, None),
        0x1b => return (Escape, None),
        0x98 | 0x9e..=0x9f => return (SosPmApcString, None),
        0x9b => return (CsiEntry, None),
        0x90 => return (DcsEntry, None),
        0x9d => return (OscString, None),
        0..=0x17 | 0x19 | 0x1c..=0x1f => return control(byte, state),
        _ => {}
    }
    match state {
        Ground if byte <= 0x7f => (state, Print),
        Escape | EscapeIntermediate => escape(byte, state),
        CsiEntry | CsiIntermediate | CsiParam | CsiIgnore => csi(byte, state),
        DcsEntry | DcsParam | DcsIntermediate | DcsPassthrough | DcsIgnore => dcs(byte, state),
        SosPmApcString if byte <= 0x7f => (state, ApcPut),
        _ => (state, None),
    }
}

/// Resolves non-global C0 bytes, including OSC's BEL terminator.
const fn control(byte: u16, state: State) -> (State, TransitionAction) {
    let action = match state {
        OscString if byte == 7 => return (Ground, None),
        OscString | DcsEntry | DcsParam | DcsIntermediate | DcsIgnore => Ignore,
        DcsPassthrough => Put,
        SosPmApcString => ApcPut,
        _ => Execute,
    };
    (state, action)
}

/// Resolves escape dispatch, intermediate collection and string introducers.
const fn escape(byte: u16, state: State) -> (State, TransitionAction) {
    match (state, byte) {
        (_, 0x20..=0x2f) => (EscapeIntermediate, Collect),
        (_, 0x7f) => (state, Ignore),
        (EscapeIntermediate, 0x30..=0x7e) => (Ground, EscDispatch),
        (Escape, 0x30..=0x4f | 0x51..=0x57 | 0x59..=0x5a | 0x5c | 0x60..=0x7e) => {
            (Ground, EscDispatch)
        }
        (Escape, 0x58 | 0x5e..=0x5f) => (SosPmApcString, None),
        (Escape, 0x50) => (DcsEntry, None),
        (Escape, 0x5b) => (CsiEntry, None),
        (Escape, 0x5d) => (OscString, None),
        _ => (state, None),
    }
}

/// Resolves CSI parameter, intermediate, ignore and dispatch branches.
const fn csi(byte: u16, state: State) -> (State, TransitionAction) {
    match (state, byte) {
        (_, 0x7f) | (CsiIgnore, 0x20..=0x3f) => (state, Ignore),
        (CsiIgnore, 0x40..=0x7e) => (Ground, None),
        (_, 0x40..=0x7e) => (Ground, CsiDispatch),
        (_, 0x20..=0x2f) => (CsiIntermediate, Collect),
        (CsiEntry, 0x30..=0x39 | 0x3b) => (CsiParam, Param),
        (CsiEntry, 0x3c..=0x3f) => (CsiParam, Collect),
        (CsiEntry, 0x3a) | (CsiParam, 0x3c..=0x3f) | (CsiIntermediate, 0x30..=0x3f) => {
            (CsiIgnore, None)
        }
        (CsiParam, 0x30..=0x3b) => (state, Param),
        _ => (state, None),
    }
}

/// Resolves DCS parameter, intermediate, ignore and payload branches.
const fn dcs(byte: u16, state: State) -> (State, TransitionAction) {
    match (state, byte) {
        (DcsIgnore, _) => (state, None),
        (_, 0x7f) => (state, Ignore),
        (DcsPassthrough, 0x20..=0x7e) => (state, Put),
        (_, 0x40..=0x7e) => (DcsPassthrough, None),
        (_, 0x20..=0x2f) => (DcsIntermediate, Collect),
        (DcsEntry | DcsParam, 0x30..=0x39 | 0x3b) => (DcsParam, Param),
        (DcsEntry, 0x3c..=0x3f) => (DcsParam, Collect),
        (DcsEntry, 0x3a) | (DcsParam, 0x3a | 0x3c..=0x3f) | (DcsIntermediate, 0x30..=0x3f) => {
            (DcsIgnore, None)
        }
        _ => (state, None),
    }
}

//! Stateful VT byte parsing with borrowed command views.

use crate::osc;

/// States of the VT byte parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Normal text input.
    Ground,
    /// Escape sequence introducer.
    Escape,
    /// Escape intermediate bytes.
    EscapeIntermediate,
    /// Control sequence introducer.
    CsiEntry,
    /// Control sequence intermediate bytes.
    CsiIntermediate,
    /// Control sequence parameters.
    CsiParam,
    /// Discarded control sequence.
    CsiIgnore,
    /// Device control introducer.
    DcsEntry,
    /// Device control parameters.
    DcsParam,
    /// Device control intermediate bytes.
    DcsIntermediate,
    /// Device control payload.
    DcsPassthrough,
    /// Discarded device control sequence.
    DcsIgnore,
    /// Operating system command payload.
    OscString,
    /// Application or privacy message payload.
    SosPmApcString,
}

/// Actions performed during a VT state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionAction {
    /// No transition action.
    None,
    /// Ignore the byte.
    Ignore,
    /// Print the byte.
    Print,
    /// Execute a control byte.
    Execute,
    /// Collect an intermediate byte.
    Collect,
    /// Accumulate a parameter byte.
    Param,
    /// Dispatch an escape sequence.
    EscDispatch,
    /// Dispatch a control sequence.
    CsiDispatch,
    /// Deliver a device control byte.
    Put,
    /// Collect an operating system command byte.
    OscPut,
    /// Deliver an application command byte.
    ApcPut,
}

/// Colon positions in the retained 24-parameter representation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SepList(pub u32);
impl SepList {
    /// Read a colon bit, ignoring positions outside the representation.
    #[must_use]
    pub const fn is_set(self, index: usize) -> bool {
        index < 24 && self.0 & (1 << index) != 0
    }
    /// Mark a colon after a retained parameter.
    pub fn set(&mut self, index: usize) {
        if index < 24 {
            self.0 |= 1 << index;
        }
    }
    /// Count retained colon positions.
    #[must_use]
    pub const fn count(self) -> u32 {
        (self.0 & 0x00ff_ffff).count_ones()
    }
}

/// Parameter separator vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sep {
    /// Ordinary parameter separator.
    Semicolon,
    /// Subparameter separator.
    Colon,
}

/// Retained intermediate-byte capacity.
pub const MAX_INTERMEDIATE: usize = 4;
/// Retained numeric parameter capacity.
pub const MAX_PARAMS: usize = 24;

/// Writable control-sequence view, valid until the next mutable parser use.
#[derive(Debug, PartialEq, Eq)]
pub struct Csi<'a> {
    /// Intermediate bytes.
    pub intermediates: &'a mut [u8],
    /// Numeric parameters.
    pub params: &'a mut [u16],
    /// Colon positions following parameters.
    pub params_sep: SepList,
    /// Final command byte.
    pub r#final: u8,
}

/// Writable escape-sequence view.
#[derive(Debug, PartialEq, Eq)]
pub struct Esc<'a> {
    /// Intermediate bytes.
    pub intermediates: &'a mut [u8],
    /// Final command byte.
    pub r#final: u8,
}

/// Read-only device-control hook view.
#[derive(Debug, PartialEq, Eq)]
pub struct Dcs<'a> {
    /// Intermediate bytes.
    pub intermediates: &'a [u8],
    /// Numeric parameters.
    pub params: &'a [u16],
    /// Final command byte.
    pub r#final: u8,
}

/// One action emitted by a VT byte.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Print a character.
    Print(char),
    /// Execute a control byte.
    Execute(u8),
    /// Dispatch a control sequence.
    CsiDispatch(Csi<'a>),
    /// Dispatch an escape sequence.
    EscDispatch(Esc<'a>),
    /// Dispatch an owned operating-system command.
    OscDispatch(osc::Command),
    /// Begin device-control payload delivery.
    DcsHook(Dcs<'a>),
    /// Deliver one device-control payload byte.
    DcsPut(u8),
    /// End device-control payload delivery.
    DcsUnhook,
    /// Begin application-command payload delivery.
    ApcStart,
    /// Deliver one application-command payload byte.
    ApcPut(u8),
    /// End application-command payload delivery.
    ApcEnd,
}

/// Fixed VT accumulators with an independently configurable OSC decoder.
pub struct Parser {
    /// Current byte-parser state.
    pub state: State,
    /// OSC capture and allocation availability.
    pub osc_parser: osc::Parser,
    /// Retained intermediate bytes.
    intermediates: [u8; MAX_INTERMEDIATE],
    /// Number of retained intermediates.
    intermediates_len: usize,
    /// Retained numeric parameters.
    params: [u16; MAX_PARAMS],
    /// Number of committed parameters.
    params_len: usize,
    /// Colon positions in committed parameters.
    params_sep: SepList,
    /// Saturating current numeric value.
    param_acc: u16,
    /// Presence of digits, independent of numeric value and input length.
    numeric_present: bool,
}

impl Parser {
    /// Start in ground state with fixed OSC capture.
    #[must_use]
    pub const fn init() -> Self {
        Self {
            state: State::Ground,
            osc_parser: osc::Parser::init(false),
            intermediates: [0; MAX_INTERMEDIATE],
            intermediates_len: 0,
            params: [0; MAX_PARAMS],
            params_len: 0,
            params_sep: SepList(0),
            param_acc: 0,
            numeric_present: false,
        }
    }

    /// Return the byte's ordered exit, transition and entry actions.
    pub fn next(&mut self, c: u8) -> [Option<Action<'_>>; 3] {
        let effect = crate::parse_table::TABLE[usize::from(c)][self.state as usize];
        let changed = self.state != effect.state;
        let exit = if changed {
            match self.state {
                State::OscString => self.osc_parser.end(Some(c)).map(Action::OscDispatch),
                State::DcsPassthrough => Some(Action::DcsUnhook),
                State::SosPmApcString => Some(Action::ApcEnd),
                _ => None,
            }
        } else {
            None
        };
        let transition = self.transition(effect.action, c);
        self.state = effect.state;
        if changed {
            match self.state {
                State::Escape | State::DcsEntry | State::CsiEntry => self.clear(),
                State::OscString => self.osc_parser.reset(),
                _ => (),
            }
        }
        if changed && self.state == State::DcsPassthrough {
            let entry = if self.finalize_params() {
                Some(Action::DcsHook(Dcs {
                    intermediates: &self.intermediates[..self.intermediates_len],
                    params: &self.params[..self.params_len],
                    r#final: c,
                }))
            } else {
                None
            };
            return [exit, None, entry];
        }
        let entry = if changed && self.state == State::SosPmApcString {
            Some(Action::ApcStart)
        } else {
            None
        };
        [exit, self.emit(transition, c), entry]
    }

    /// Retain an intermediate byte while fixed capacity permits.
    pub fn collect(&mut self, c: u8) {
        if let Some(slot) = self.intermediates.get_mut(self.intermediates_len) {
            *slot = c;
            self.intermediates_len += 1;
        }
    }

    /// Clear accumulation before a new escape/control sequence.
    pub fn clear(&mut self) {
        self.intermediates_len = 0;
        self.params_len = 0;
        self.params_sep = SepList(0);
        self.param_acc = 0;
        self.numeric_present = false;
    }

    /// Accumulate decimal digits or commit a separator once.
    fn parameter(&mut self, c: u8) {
        if !matches!(c, b';' | b':') {
            self.param_acc = self
                .param_acc
                .saturating_mul(10)
                .saturating_add(u16::from(c - b'0'));
            self.numeric_present = true;
            return;
        }
        let Some(slot) = self.params.get_mut(self.params_len) else {
            return;
        };
        *slot = self.param_acc;
        if c == b':' {
            self.params_sep.set(self.params_len);
        }
        self.params_len += 1;
        self.param_acc = 0;
        self.numeric_present = false;
    }

    /// Finalize a present value unless the committed array already overflowed.
    fn finalize_params(&mut self) -> bool {
        if self.params_len >= MAX_PARAMS {
            return false;
        }
        if self.numeric_present {
            self.params[self.params_len] = self.param_acc;
            self.params_len += 1;
        }
        true
    }

    /// Apply accumulation before borrowing the resulting dispatch view.
    fn transition(&mut self, action: TransitionAction, c: u8) -> Option<TransitionAction> {
        match action {
            TransitionAction::None | TransitionAction::Ignore => None,
            TransitionAction::Collect => {
                self.collect(c);
                None
            }
            TransitionAction::Param => {
                self.parameter(c);
                None
            }
            TransitionAction::OscPut => {
                self.osc_parser.next(c);
                None
            }
            TransitionAction::CsiDispatch => {
                if !self.finalize_params() || (c != b'm' && self.params_sep.count() > 0) {
                    return None;
                }
                Some(action)
            }
            _ => Some(action),
        }
    }

    /// Borrow a dispatch view only after all state-entry effects complete.
    fn emit(&mut self, action: Option<TransitionAction>, c: u8) -> Option<Action<'_>> {
        match action? {
            TransitionAction::Print => Some(Action::Print(char::from(c))),
            TransitionAction::Execute => Some(Action::Execute(c)),
            TransitionAction::Put => Some(Action::DcsPut(c)),
            TransitionAction::ApcPut => Some(Action::ApcPut(c)),
            TransitionAction::EscDispatch => Some(Action::EscDispatch(Esc {
                intermediates: &mut self.intermediates[..self.intermediates_len],
                r#final: c,
            })),
            TransitionAction::CsiDispatch => Some(Action::CsiDispatch(Csi {
                intermediates: &mut self.intermediates[..self.intermediates_len],
                params: &mut self.params[..self.params_len],
                params_sep: self.params_sep,
                r#final: c,
            })),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;

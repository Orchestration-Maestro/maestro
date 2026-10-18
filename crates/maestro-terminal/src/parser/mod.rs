//! VT parser state and transition action vocabulary.

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

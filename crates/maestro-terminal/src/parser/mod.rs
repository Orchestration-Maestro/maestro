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

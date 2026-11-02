//! The signal delivery and process exit of a print run.
use maestro_app::{TerminationSignal, register_signals};
use std::{io, rc::Rc};

/// The signal delivery and process exit of a print run.
pub trait SignalPort {
    /// Delivers each termination signal to `callback`; the returned operation stops the
    /// delivery.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn register(&self, callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>>;

    /// Ends the process with `code`.
    fn exit(&self, code: i32);
}

/// The host's own signal delivery and process exit.
pub struct NativeSignals;

impl SignalPort for NativeSignals {
    fn register(&self, callback: Rc<dyn Fn(TerminationSignal)>) -> io::Result<Box<dyn Fn()>> {
        register_signals(callback)
    }

    fn exit(&self, code: i32) {
        std::process::exit(code);
    }
}

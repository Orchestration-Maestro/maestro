//! The registration of output-failure callbacks and the source of each native failure.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use maestro_listeners::{Admission, Listeners};
use maestro_tui::terminal::OutputErrorHandler;

/// The producer of a native failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailureSource {
    /// Reading standard input; retained, never an output failure.
    Input,
    /// Writing to standard output of the process terminal.
    #[cfg(any(unix, windows))]
    Output,
    /// Reporting on standard error.
    Stderr,
}

/// The callbacks registered for native output failures.
pub(crate) struct OutputErrors {
    /// The live callbacks, each registration kept as its own entry.
    callbacks: Rc<RefCell<Listeners<OutputErrorHandler>>>,
}

impl Default for OutputErrors {
    fn default() -> Self {
        Self {
            callbacks: Rc::new(RefCell::new(Listeners::new(Admission::Every))),
        }
    }
}

impl OutputErrors {
    /// Registers `handler`; the returned operation removes that registration and does
    /// nothing when repeated.
    pub(crate) fn register(self: &Rc<Self>, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        Box::new(Listeners::subscribe(&self.callbacks, handler))
    }

    /// Passes `error` to every registered callback, holding no borrow while they run.
    pub(crate) fn notify(&self, error: &io::Error) {
        let Ok(()) = Listeners::try_for_each_live(&self.callbacks, |handler| {
            handler(error);
            Ok::<(), std::convert::Infallible>(())
        });
    }

    /// Passes `error` to the callbacks unless it came from input, which is not an output
    /// failure.
    pub(crate) fn notify_source(&self, source: FailureSource, error: &io::Error) {
        if source != FailureSource::Input {
            self.notify(error);
        }
    }
}

#[cfg(test)]
mod tests;

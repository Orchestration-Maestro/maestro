//! The one owner of registered output-failure callbacks, and the source of each native failure.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

use maestro_tui::terminal::OutputErrorHandler;

/// The producer of a native failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailureSource {
    /// Reading standard input; retained, never an output failure.
    Input,
    /// Writing to standard output of the process terminal.
    #[cfg(unix)]
    Output,
    /// Reporting on standard error.
    Stderr,
}

/// The callbacks registered for native output failures.
#[derive(Default)]
pub(crate) struct OutputErrors {
    /// The live callbacks, each under the number that removes it.
    handlers: RefCell<Vec<(u64, OutputErrorHandler)>>,
    /// The number the next registration takes.
    next: Cell<u64>,
}

impl OutputErrors {
    /// Registers `handler`; the returned operation removes it and does nothing when repeated
    /// or when a later registration replaced it.
    pub(crate) fn register(self: &Rc<Self>, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        let slot = self.next.get();
        self.next.set(slot + 1);
        self.handlers.borrow_mut().push((slot, handler));
        let owner = Rc::clone(self);
        Box::new(move || owner.remove(slot))
    }

    /// Passes `error` to every registered callback, holding no borrow while they run.
    pub(crate) fn notify(&self, error: &io::Error) {
        let live: Vec<OutputErrorHandler> = self
            .handlers
            .borrow()
            .iter()
            .map(|(_, handler)| Rc::clone(handler))
            .collect();
        for handler in live {
            handler(error);
        }
    }

    /// Passes `error` to the callbacks unless it came from input, which is not an output
    /// failure.
    pub(crate) fn notify_source(&self, source: FailureSource, error: &io::Error) {
        if source != FailureSource::Input {
            self.notify(error);
        }
    }

    /// Drops the callback registered as `slot`, leaving a later registration in place.
    fn remove(&self, slot: u64) {
        self.handlers
            .borrow_mut()
            .retain(|(registered, _)| *registered != slot);
    }
}

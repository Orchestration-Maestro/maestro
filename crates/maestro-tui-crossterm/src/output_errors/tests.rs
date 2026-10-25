//! Event-emitter admission of the output-failure callbacks.

use std::cell::Cell;
use std::io;
use std::rc::Rc;

use maestro_tui::terminal::OutputErrorHandler;

use super::OutputErrors;

#[test]
fn a_handler_registered_twice_is_called_twice_until_one_removal() {
    let errors = Rc::new(OutputErrors::default());
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::clone(&calls);
    let handler: OutputErrorHandler = Rc::new(move |_| seen.set(seen.get() + 1));
    let first = errors.register(Rc::clone(&handler));
    let _second = errors.register(handler);
    let error = io::Error::other("closed");
    errors.notify(&error);
    assert_eq!(calls.get(), 2);
    first();
    first();
    errors.notify(&error);
    assert_eq!(calls.get(), 3);
}

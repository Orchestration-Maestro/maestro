//! Owned views of the caller's text streams that borrow them only per write.
use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

/// The caller's existing writer, borrowed only for each individual write or flush.
///
/// No borrow outlives one call, so callbacks reached later can write to the same stream.
pub(super) struct SharedWriter(pub(super) Rc<RefCell<dyn Write>>);

impl Write for SharedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

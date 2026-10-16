//! Ownership and byte-exact routing for application-managed standard output.
use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

/// Whether ordinary managed output belongs on standard error.
static STDOUT_TAKEN_OVER: AtomicBool = AtomicBool::new(false);

/// Reserve standard output for protocol traffic.
pub fn take_over_stdout() {
    STDOUT_TAKEN_OVER.store(true, Ordering::SeqCst);
}

/// Return ordinary managed output to standard output.
pub fn restore_stdout() {
    STDOUT_TAKEN_OVER.store(false, Ordering::SeqCst);
}

/// Whether standard output is reserved for protocol traffic.
#[must_use]
pub fn is_stdout_taken_over() -> bool {
    STDOUT_TAKEN_OVER.load(Ordering::SeqCst)
}

/// Write ordinary output to the currently selected stream without adding a newline.
/// # Errors
/// Returns the selected stream's write failure.
pub fn write_stdout(text: &str) -> io::Result<()> {
    if is_stdout_taken_over() {
        io::stderr().write_all(text.as_bytes())
    } else {
        write_raw_stdout(text)
    }
}

/// Write protocol bytes to standard output regardless of ownership.
/// # Errors
/// Returns the standard output write failure.
pub fn write_raw_stdout(text: &str) -> io::Result<()> {
    io::stdout().write_all(text.as_bytes())
}

/// Complete a synchronous flush of standard output regardless of ownership.
/// # Errors
/// Returns the standard output flush failure.
pub fn flush_raw_stdout() -> io::Result<()> {
    io::stdout().flush()
}

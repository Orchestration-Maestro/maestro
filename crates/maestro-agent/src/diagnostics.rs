//! Diagnostics written to standard error that never fail their caller.
use std::io::{self, Write};

/// Writes `text` to standard error exactly as given.
///
/// A diagnostic is a courtesy: when standard error is closed or full, the write is
/// dropped and the caller carries on.
pub fn write_stderr(text: &str) {
    let _ = io::stderr().write_all(text.as_bytes());
}

/// Writes `text` and a final newline to standard error in one write.
///
/// A failed write is dropped, as in [`write_stderr`].
pub fn write_stderr_line(text: &str) {
    write_stderr(&format!("{text}\n"));
}

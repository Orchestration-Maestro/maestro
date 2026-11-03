//! Recording console effects with selectable write or flush failure.
use std::io::{self, Write};

/// Console operation that refuses its first and every later attempt.
#[derive(Clone, Copy)]
pub enum Failure {
    /// Refuse a byte write.
    Write,
    /// Refuse a flush.
    Flush,
}

/// Console calls retained for assertions while a real session emits events.
#[derive(Default)]
pub struct RecordingWriter {
    /// Ordered byte writes and flushes, including failed attempts.
    pub calls: Vec<String>,
    /// Optional operation to refuse.
    pub failure: Option<Failure>,
}

impl Write for RecordingWriter {
    /// Record a byte write before optionally refusing it.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls.push(String::from_utf8_lossy(bytes).into_owned());
        if matches!(self.failure, Some(Failure::Write)) {
            return Err(io::Error::other(format!(
                "write failure {}",
                self.calls.len()
            )));
        }
        Ok(bytes.len())
    }

    /// Record a flush before optionally refusing it.
    fn flush(&mut self) -> io::Result<()> {
        self.calls.push("flush".into());
        if matches!(self.failure, Some(Failure::Flush)) {
            return Err(io::Error::other(format!(
                "flush failure {}",
                self.calls.len()
            )));
        }
        Ok(())
    }
}

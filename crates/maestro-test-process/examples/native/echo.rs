//! Controlled program that prints the line its sibling record authors.

use std::ffi::OsString;
use std::io;

use maestro_test_process::child::{leave, write_stdout};

/// The record of a controlled print, whose text the caller authors.
pub(super) struct Record {
    /// The exact line the program prints.
    text: String,
}

impl Record {
    /// Read a print record, rejecting missing or extra fields.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["echo", line] = fields.as_slice() else {
            return Err(io::Error::other("expected echo and one line"));
        };
        Ok(Self {
            text: (*line).to_owned(),
        })
    }
}

/// Print the authored line and end.
pub(super) fn run(record: &Record, _operands: &[OsString]) -> io::Result<()> {
    write_stdout(format!("{}\n", record.text).as_bytes())?;
    leave(0)
}

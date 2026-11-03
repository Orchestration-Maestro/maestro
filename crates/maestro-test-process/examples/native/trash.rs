//! Controlled trash program selected by a sibling JSON record.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use maestro_test_process::child::{leave, write_stderr};
use serde::Deserialize;

/// Trash outcomes parsed completely before the operands are read.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// Destination receiving each literal operand on its own line.
    log: Option<String>,
    /// Captured diagnostic bytes, which may hold several lines.
    #[serde(default)]
    stderr: Vec<u8>,
    /// Remove the first operand instead of moving it.
    #[serde(default)]
    remove: bool,
    /// Exit status after the recorded effects.
    #[serde(default)]
    code: u8,
}

impl Record {
    /// Require the family and every effect before the first log write.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("trash\n")
            .ok_or_else(|| io::Error::other("missing trash family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Write each operand on its own line, without interpreting an option-like name.
fn log_operands(path: &str, operands: &[OsString]) -> io::Result<()> {
    let mut text = String::new();
    for operand in operands {
        text.push_str(&operand.to_string_lossy());
        text.push('\n');
    }
    std::fs::write(path, text)
}

/// Record the literal operands, optionally remove the file, then end with the status.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    if let Some(log) = &record.log {
        log_operands(log, operands)?;
    }
    if record.remove {
        let path = operands
            .first()
            .ok_or_else(|| io::Error::other("missing trash operand"))?;
        std::fs::remove_file(Path::new(path))?;
    }
    write_stderr(&record.stderr)?;
    leave(i32::from(record.code))
}

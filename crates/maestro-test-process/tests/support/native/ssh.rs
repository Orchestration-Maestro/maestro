//! A stand-in remote client selected by a sibling record.

use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro_test_process::child::{leave, write_stderr};
use serde::Deserialize;

/// The status every launch without a destination ends with.
const REFUSED: i32 = 2;

/// The one destination the stand-in refuses, with its status and diagnostic.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Refusal {
    /// The destination every command is refused for.
    destination: String,
    /// Status the refusal ends with.
    code: u8,
    /// Diagnostic the refusal writes before it ends.
    stderr: String,
}

/// The remote command whose body is refused, and the refusal of it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Write {
    /// The remote command whose standard input the stand-in never reads.
    command: String,
    /// Status of the refusal.
    code: u8,
    /// Diagnostic of the refusal.
    stderr: String,
}

/// The three behaviours of the stand-in, read before any effect.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// The destination the stand-in refuses.
    refusal: Refusal,
    /// The remote write the stand-in refuses unread.
    write: Write,
    /// The remote session's working directory, which every accepted command runs in.
    directory: PathBuf,
}

impl Record {
    /// Reject an unknown family, extra fields and malformed values before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("ssh\n")
            .ok_or_else(|| io::Error::other("missing ssh family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// The destination and the remote command of one launch.
///
/// Options are skipped while they stand alone; an option taking a separate value is not read, so
/// a caller passes the destination before its command.
fn split(operands: &[OsString]) -> Option<(&str, String)> {
    let mut operands = operands
        .iter()
        .skip_while(|operand| operand.to_string_lossy().starts_with('-'));
    let destination = operands.next()?.to_str()?;
    let command = operands
        .map(|word| word.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    Some((destination, command))
}

/// Write the diagnostic and leave with the refusal's status.
fn refuse(code: u8, stderr: &str) -> io::Result<()> {
    write_stderr(stderr.as_bytes())?;
    leave(i32::from(code))
}

/// Replace this process with the real shell running `command` in `directory`.
fn shell(directory: &Path, command: &str) -> io::Result<()> {
    Err(Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(directory)
        .exec())
}

/// Refuse the recorded destination or write, else run the command in the session.
///
/// A caller streams a write's body through a pipe, which reports no length, so the record
/// names the command rather than the size it refuses.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    let Some((destination, command)) = split(operands) else {
        leave(REFUSED);
    };
    if destination == record.refusal.destination {
        return refuse(record.refusal.code, &record.refusal.stderr);
    }
    if command.contains(&record.write.command) {
        return refuse(record.write.code, &record.write.stderr);
    }
    shell(&record.directory, &command)
}

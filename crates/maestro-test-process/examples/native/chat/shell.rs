//! The configured shell of the interactive mode: finite steps run for each command it is given.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use maestro_test_process::child::{leave, write_stdout};
use serde::Deserialize;

/// A path a step names.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Target {
    /// This exact path.
    Path(String),
    /// The command text followed by this suffix.
    Command(String),
}

/// One effect of a shell command.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Step {
    /// Write this text to standard output.
    Print(String),
    /// Write the command text between `before` and `after` to standard output.
    PrintCommand {
        /// Text written before the command.
        before: String,
        /// Text written after the command.
        after: String,
    },
    /// Create an empty file.
    Mark(Target),
    /// Wait on the gate the test listens on at this path until the test releases it.
    Hold(Target),
    /// Append one empty line to this file.
    Log(String),
    /// Wait until a signal ends the process.
    Block,
    /// End with this status.
    Exit(u8),
}

/// The text of the command: the last operand of `shell -c <command>`.
fn command_text(operands: &[OsString]) -> io::Result<&OsString> {
    operands
        .last()
        .ok_or_else(|| io::Error::other("the shell was given no command"))
}

/// The path a target names for this command.
fn path(target: &Target, operands: &[OsString]) -> io::Result<OsString> {
    match target {
        Target::Path(path) => Ok(path.into()),
        Target::Command(suffix) => {
            let mut path = command_text(operands)?.clone();
            path.push(suffix);
            Ok(path)
        }
    }
}

/// Wait until the test releases the gate: it closes its listener, which resets a connection
/// still queued and refuses a later one, so every release ends the wait.
fn hold(gate: &OsString) {
    if let Ok(mut gate) = UnixStream::connect(gate) {
        // Any outcome of the read, data, end or reset, is the release.
        let _ = gate.read(&mut [0]);
    }
}

/// Wait for a signal on a connection nobody writes to.
fn block() -> io::Result<()> {
    let (mut reader, _writer) = UnixStream::pair()?;
    reader.read_exact(&mut [0])
}

/// Run each step in order, then end successfully.
pub(super) fn run(steps: &[Step], operands: &[OsString]) -> io::Result<()> {
    for step in steps {
        match step {
            Step::Print(text) => write_stdout(text.as_bytes())?,
            Step::PrintCommand { before, after } => write_stdout(
                &[
                    before.as_bytes(),
                    command_text(operands)?.as_encoded_bytes(),
                    after.as_bytes(),
                ]
                .concat(),
            )?,
            Step::Mark(target) => drop(std::fs::File::create(path(target, operands)?)?),
            Step::Hold(target) => hold(&path(target, operands)?),
            Step::Log(log) => std::fs::File::options()
                .append(true)
                .create(true)
                .open(log)?
                .write_all(b"\n")?,
            Step::Block => block()?,
            Step::Exit(code) => leave(i32::from(*code)),
        }
    }
    leave(0)
}

//! Stand-in Git behavior selected by a sibling record.

use std::io;
use std::process::{Command, Stdio};

use maestro_test_process::child::{announce, connect, leave, receive, write_stderr, write_stdout};
use maestro_test_process::git_command::Scenario;

/// The descriptors that report where the product pointed the launched stand-in.
const DESCRIPTORS: [&str; 2] = ["/proc/self/fd/0", "/proc/self/fd/2"];
/// The branch a successful observed lookup answers.
const BRANCH: &[u8] = b"feature/native\n";
/// The diagnostic an observed lookup writes before its answer.
const DIAGNOSTIC: &[u8] = b"noise\n";
/// The output the exiting stand-in writes before it leaves.
const PREFIX: &[u8] = b" prefix";
/// The output the retaining descendant adds once the test releases it.
const TAIL: &[u8] = b" tail\n";

/// A stand-in request whose external inputs have been read before effects.
pub(super) enum Record {
    /// Log the native query in this file, then answer with this status.
    Observed(String, i32),
    /// Print the native query, then answer with this status.
    Reported(i32),
    /// Write a prefix and leave a descendant holding the output descriptor.
    Exiting,
    /// Hold the inherited output descriptor until the test releases it.
    Retaining,
}

impl Record {
    /// Require a known scenario and read its complete input before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["git", scenario] = fields.as_slice() else {
            return Err(io::Error::other("expected git and a scenario"));
        };
        Ok(match scenario.parse::<Scenario>()? {
            Scenario::Observed => Self::Observed(variable("GIT_LOG")?, status()?),
            Scenario::Reported => Self::Reported(status()?),
            Scenario::Exiting => Self::Exiting,
            Scenario::Retaining => Self::Retaining,
        })
    }
}

/// The operands and directory this launch supplied, each followed by a newline.
fn location() -> io::Result<String> {
    let operands = std::env::args_os()
        .skip(1)
        .map(|operand| operand.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(format!(
        "{operands}\n{}\n",
        std::env::current_dir()?.display()
    ))
}

/// Read one required text input from the isolated child's environment.
fn variable(name: &str) -> io::Result<String> {
    std::env::var(name).map_err(io::Error::other)
}

/// The status in `GIT_EXIT`, which defaults to a successful lookup.
fn status() -> io::Result<i32> {
    match std::env::var("GIT_EXIT") {
        Ok(text) => text.parse::<u8>().map(i32::from).map_err(io::Error::other),
        Err(std::env::VarError::NotPresent) => Ok(0),
        Err(error) => Err(io::Error::other(error)),
    }
}

/// Log the native query with its descriptor targets, then answer the lookup.
fn observed(log: &str, exit: i32) -> io::Result<()> {
    let mut text = location()?;
    // The descriptor targets exist where the platform publishes them for the running process.
    if cfg!(target_os = "linux") {
        for descriptor in DESCRIPTORS {
            text.push_str(&std::fs::read_link(descriptor)?.display().to_string());
            text.push('\n');
        }
    }
    std::fs::write(log, text)?;
    write_stderr(DIAGNOSTIC)?;
    write_stdout(BRANCH)?;
    leave(exit)
}

/// Print the native query, then answer the lookup.
fn reported(exit: i32) -> io::Result<()> {
    write_stdout(location()?.as_bytes())?;
    leave(exit)
}

/// Write the prefix, start the retaining descendant and leave without waiting for it.
fn exiting() -> io::Result<()> {
    write_stdout(PREFIX)?;
    let port = std::fs::read_to_string("holder-port")?;
    Command::new(variable("GIT_HOLDER")?)
        .env("GIT_PORT", port.trim())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    leave(0)
}

/// Announce this descendant, then complete the retained output once released.
fn retaining() -> io::Result<()> {
    let port = variable("GIT_PORT")?
        .parse::<u16>()
        .map_err(io::Error::other)?;
    let mut control = connect(port)?;
    announce(&mut control, &std::process::id().to_string())?;
    if receive(&mut control)? != Some(b'!') {
        return Err(io::Error::other("expected the footer release byte"));
    }
    write_stdout(TAIL)
}

/// Execute the parsed behavior on the actual child's standard streams.
pub(super) fn run(record: &Record) -> io::Result<()> {
    match record {
        Record::Observed(log, exit) => observed(log, *exit),
        Record::Reported(exit) => reported(*exit),
        Record::Exiting => exiting(),
        Record::Retaining => retaining(),
    }
}

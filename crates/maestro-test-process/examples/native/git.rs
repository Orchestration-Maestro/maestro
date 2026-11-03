//! Stand-in Git behavior selected by a sibling record.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};

#[cfg(unix)]
use maestro_test_process::child::{announce, connect, receive, write_stderr};
use maestro_test_process::child::{leave, write_stdout};
#[cfg(unix)]
use maestro_test_process::git_command::Scenario;
use serde::Deserialize;

/// The descriptors that report where the product pointed the launched stand-in.
#[cfg(unix)]
const DESCRIPTORS: [&str; 2] = ["/proc/self/fd/0", "/proc/self/fd/2"];
/// The branch a successful observed lookup answers.
#[cfg(unix)]
const BRANCH: &[u8] = b"feature/native\n";
/// The diagnostic an observed lookup writes before its answer.
#[cfg(unix)]
const DIAGNOSTIC: &[u8] = b"noise\n";
/// The output the exiting stand-in writes before it leaves.
#[cfg(unix)]
const PREFIX: &[u8] = b" prefix";
/// The output the retaining descendant adds once the test releases it.
#[cfg(unix)]
const TAIL: &[u8] = b" tail\n";

/// The status every query outside the finite repository probe ends with.
const REFUSED: i32 = 2;
/// The symbolic branch the probe reports, with the surrounding spaces Git readers trim.
const TOPIC: &[u8] = b" topic\n";
/// The object identifier the probe reports for the existing reference.
const OBJECT: &[u8] = b" 0123456789abcdef\n";
/// Output that must be discarded when the query fails.
const DISCARDED: &[u8] = b"discard this output\n";

/// The checkout every repository probe query must name through `-C`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Probe {
    /// The exact `-C` directory of every accepted query.
    cwd: PathBuf,
}

/// A stand-in request whose external inputs have been read before effects.
pub(super) enum Record {
    /// Answer the finite reftable repository queries for this checkout.
    Probe(Probe),
    /// Log the native query in this file, then answer with this status.
    #[cfg(unix)]
    Observed(String, i32),
    /// Print the native query, then answer with this status.
    #[cfg(unix)]
    Reported(i32),
    /// Write a prefix and leave a descendant holding the output descriptor.
    #[cfg(unix)]
    Exiting,
    /// Hold the inherited output descriptor until the test releases it.
    #[cfg(unix)]
    Retaining,
}

impl Record {
    /// Require a known scenario and read its complete input before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        if let Some(json) = text.strip_prefix("git\nProbe\n") {
            return serde_json::from_str(json)
                .map(Self::Probe)
                .map_err(io::Error::other);
        }
        #[cfg(unix)]
        {
            Self::footer(text)
        }
        #[cfg(not(unix))]
        {
            Err(io::Error::other("unknown Git stand-in scenario"))
        }
    }

    /// Read a Unix footer scenario and its isolated environment before effects.
    #[cfg(unix)]
    fn footer(text: &str) -> io::Result<Self> {
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
#[cfg(unix)]
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
#[cfg(unix)]
fn variable(name: &str) -> io::Result<String> {
    std::env::var(name).map_err(io::Error::other)
}

/// The status in `GIT_EXIT`, which defaults to a successful lookup.
#[cfg(unix)]
fn status() -> io::Result<i32> {
    match std::env::var("GIT_EXIT") {
        Ok(text) => text.parse::<u8>().map(i32::from).map_err(io::Error::other),
        Err(std::env::VarError::NotPresent) => Ok(0),
        Err(error) => Err(io::Error::other(error)),
    }
}

/// Log the native query with its descriptor targets, then answer the lookup.
#[cfg(unix)]
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
#[cfg(unix)]
fn reported(exit: i32) -> io::Result<()> {
    write_stdout(location()?.as_bytes())?;
    leave(exit)
}

/// Write the prefix, start the retaining descendant and leave without waiting for it.
#[cfg(unix)]
fn exiting() -> io::Result<()> {
    write_stdout(PREFIX)?;
    let port = std::fs::read_to_string("holder-port")?;
    Command::new(variable("GIT_HOLDER")?)
        .env("GIT_PARENT", std::process::id().to_string())
        .env("GIT_PORT", port.trim())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    leave(0)
}

/// Announce the parent whose exit the observer awaits, then complete the retained output once
/// released.
#[cfg(unix)]
fn retaining() -> io::Result<()> {
    let parent = variable("GIT_PARENT")?;
    let port = variable("GIT_PORT")?
        .parse::<u16>()
        .map_err(io::Error::other)?;
    let mut control = connect(port)?;
    announce(&mut control, parent.trim())?;
    if receive(&mut control)? != Some(b'!') {
        return Err(io::Error::other("expected the footer release byte"));
    }
    write_stdout(TAIL)
}

/// Answer one of three finite queries and refuse every other operand sequence.
fn probe(record: &Probe, operands: &[OsString]) -> io::Result<()> {
    let Ok(words) = words(operands) else {
        leave(REFUSED)
    };
    let [flag, directory, query @ ..] = words.as_slice() else {
        leave(REFUSED)
    };
    if *flag != "-C" || Path::new(*directory) != record.cwd.as_path() {
        leave(REFUSED);
    }
    match query {
        ["symbolic-ref", "--quiet", "--short", "HEAD"] => answer(TOPIC, 0),
        ["rev-parse", "--verify", "HEAD"] => answer(OBJECT, 0),
        ["rev-parse", "--verify", "missing"] => answer(DISCARDED, 1),
        _ => leave(REFUSED),
    }
}

/// Split operands into text; a non-text word can name no accepted query.
fn words(operands: &[OsString]) -> io::Result<Vec<&str>> {
    operands
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| io::Error::other("non-text operand"))
        })
        .collect()
}

/// Write one answer and leave with its status.
fn answer(bytes: &[u8], code: i32) -> io::Result<()> {
    write_stdout(bytes)?;
    leave(code)
}

/// Execute the parsed behavior on the actual child's standard streams.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Probe(record) => probe(record, operands),
        #[cfg(unix)]
        Record::Observed(log, exit) => observed(log, *exit),
        #[cfg(unix)]
        Record::Reported(exit) => reported(*exit),
        #[cfg(unix)]
        Record::Exiting => exiting(),
        #[cfg(unix)]
        Record::Retaining => retaining(),
    }
}

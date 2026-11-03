//! Finite controlled program stand-ins selected by a sibling JSON record.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};

use maestro_test_process::child::{leave, variable, write_stderr, write_stdout};
#[cfg(unix)]
use rustix::process::{Signal, getpid, kill_process};
use serde::Deserialize;

/// Native termination instead of a numeric status.
#[derive(Deserialize)]
pub(super) enum ExitSignal {
    /// Terminate with the native SIGTERM.
    #[serde(rename = "SIGTERM")]
    Terminate,
    /// Terminate with a Linux real-time signal, which no shared name reports.
    #[cfg(target_os = "linux")]
    #[serde(rename = "SIGRTMAX")]
    Realtime,
}

/// The last Linux real-time signal, which ends a process without a name.
#[cfg(target_os = "linux")]
const REALTIME_SIGNAL: i32 = 64;

/// One append a stand-in performs before it reports.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Append {
    /// Destination appended to, created when it is absent.
    path: String,
    /// Append the working directory and a newline before the recorded bytes.
    cwd: bool,
    /// Bytes appended last.
    bytes: Vec<u8>,
}

/// One operand log a stand-in appends to.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArgvLog {
    /// Destination appended to, created when it is absent.
    path: String,
    /// Text that opens each recorded invocation.
    mark: String,
}

/// Program scenarios, parsed completely before any file or stream effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Write independent raw streams and end with the selected status or signal.
    Output {
        /// Standard output bytes.
        stdout: Vec<u8>,
        /// Standard error bytes.
        stderr: Vec<u8>,
        /// Status when no signal is selected.
        code: u8,
        /// Native termination instead of a numeric status.
        signal: Option<ExitSignal>,
        /// Optional append this invocation performs first.
        append: Option<Append>,
        /// Optional operand log this invocation appends to first.
        argv_log: Option<ArgvLog>,
    },
    /// Report the first operand on both streams, then echo one line of input.
    #[cfg(unix)]
    Labelled {},
    /// Answer a package-manager root query for the first operand.
    Query {
        /// Bytes written before the reported value.
        prefix: Vec<u8>,
        /// The operand value that reports the recorded error instead.
        denied: String,
        /// Standard error bytes for the denied value.
        error: Vec<u8>,
        /// Status for the denied value.
        code: u8,
    },
    /// Record one package-manager command and answer it.
    Package {},
    /// Answer a package-manager query that receives its prefix as the first operands.
    Prefixed {
        /// Text appended to the prefix operand for a root query.
        suffix: String,
        /// File that records every other invocation's operands, one per line.
        log: String,
    },
}

impl Record {
    /// Reject unknown scenarios and extra fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("command\n")
            .ok_or_else(|| io::Error::other("missing command family"))?;
        let record: Self = serde_json::from_str(json).map_err(io::Error::other)?;
        if !cfg!(unix)
            && matches!(
                record,
                Self::Output {
                    signal: Some(_),
                    ..
                }
            )
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "termination signal requires Unix",
            ));
        }
        Ok(record)
    }
}

/// End this process by the native SIGTERM.
#[cfg(unix)]
fn terminate() -> io::Result<()> {
    kill_process(getpid(), Signal::TERM).map_err(Into::into)
}

/// Report that this platform cannot deliver a Unix termination signal.
#[cfg(not(unix))]
fn terminate() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "termination signal requires Unix",
    ))
}

/// Run the selected stand-in with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Output {
            stdout,
            stderr,
            code,
            signal,
            append,
            argv_log,
        } => {
            if let Some(append) = append {
                append_to(append)?;
            }
            if let Some(log) = argv_log {
                record_argv(log, operands)?;
            }
            write_stdout(stdout)?;
            write_stderr(stderr)?;
            match signal {
                Some(ExitSignal::Terminate) => terminate()?,
                #[cfg(target_os = "linux")]
                Some(ExitSignal::Realtime) => {
                    signal_hook::low_level::raise(REALTIME_SIGNAL).map_err(io::Error::other)?;
                }
                None => {}
            }
            leave(i32::from(*code))
        }
        #[cfg(unix)]
        Record::Labelled {} => labelled(operands),
        Record::Query {
            prefix,
            denied,
            error,
            code,
        } => query(operands, prefix, denied, error, *code),
        Record::Package {} => package(operands),
        Record::Prefixed { suffix, log } => prefixed(operands, suffix, log),
    }
}

/// Append the working directory and then the recorded bytes to one file.
fn append_to(append: &Append) -> io::Result<()> {
    let mut line = Vec::new();
    if append.cwd {
        line.extend_from_slice(
            &std::env::current_dir()?
                .into_os_string()
                .into_encoded_bytes(),
        );
        line.push(b'\n');
    }
    line.extend_from_slice(&append.bytes);
    append_bytes(&append.path, &line)
}

/// Append the invocation mark and each untouched operand, separated by NUL.
fn record_argv(log: &ArgvLog, operands: &[OsString]) -> io::Result<()> {
    let mut line = Vec::new();
    let mark = OsStr::new(&log.mark);
    for field in std::iter::once(mark).chain(operands.iter().map(OsString::as_os_str)) {
        line.extend_from_slice(field.as_encoded_bytes());
        line.push(0);
    }
    append_bytes(&log.path, &line)
}

/// Append `bytes` in one write, so a concurrent invocation cannot interleave inside them.
fn append_bytes(path: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::File::options()
        .append(true)
        .create(true)
        .open(path)?;
    file.write_all(bytes)
}

/// Echo the first operand on both streams, then one line of standard input.
#[cfg(unix)]
fn labelled(operands: &[OsString]) -> io::Result<()> {
    let Some(label) = operands.first() else {
        return Err(io::Error::other("missing labelled operand"));
    };
    let label = label.to_string_lossy();
    write_stdout(format!("{label}\n").as_bytes())?;
    write_stderr(format!("{label}-err\n").as_bytes())?;
    let line = read_line()?;
    write_stdout(format!("stdin:{}\n", String::from_utf8_lossy(&line)).as_bytes())
}

/// One line of standard input, read without consuming bytes past its newline.
#[cfg(unix)]
fn read_line() -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    let mut byte = [0; 1];
    loop {
        match rustix::io::read(rustix::stdio::stdin(), &mut byte) {
            // An ended or unreadable input reports the value read so far, as a shell's
            // failed `read` does.
            Ok(0) | Err(_) => return Ok(line),
            Ok(_) if byte[0] == b'\n' => return Ok(line),
            Ok(_) => line.push(byte[0]),
        }
    }
}

/// Report a package root for `value`, or the recorded refusal for the denied value.
fn query(
    operands: &[OsString],
    prefix: &[u8],
    denied: &str,
    error: &[u8],
    code: u8,
) -> io::Result<()> {
    let [value, command, scope] = operands else {
        return Err(io::Error::other("expected a value followed by `root -g`"));
    };
    if command.as_os_str() != OsStr::new("root") || scope.as_os_str() != OsStr::new("-g") {
        return Err(io::Error::other("expected the `root -g` operands"));
    }
    if value.as_os_str() == OsStr::new(denied) {
        write_stderr(error)?;
        leave(i32::from(code));
    }
    write_stdout(prefix)?;
    write_stdout(value.as_encoded_bytes())?;
    write_stdout(b"\n")?;
    leave(0)
}

/// Append `directory`, the invoked name and every operand, then answer the command.
fn package(operands: &[OsString]) -> io::Result<()> {
    let log =
        variable("MAESTRO_COMMAND_LOG").map_err(|error| io::Error::other(error.to_string()))?;
    let root = variable("MAESTRO_NPM_ROOT").map_err(|error| io::Error::other(error.to_string()))?;
    let mut line = std::env::current_dir()?
        .into_os_string()
        .into_encoded_bytes();
    let invoked = std::env::args_os().next().unwrap_or_default();
    for field in
        std::iter::once(invoked.as_os_str()).chain(operands.iter().map(OsString::as_os_str))
    {
        line.push(b'\t');
        line.extend_from_slice(field.as_encoded_bytes());
    }
    line.push(b'\n');
    append_bytes(&log, &line)?;
    if operands
        .first()
        .is_none_or(|quiet| quiet.as_os_str() != OsStr::new("--quiet"))
    {
        leave(1);
    }
    match operands.get(1).and_then(|command| command.to_str()) {
        Some("root") => write_stdout(format!("{root}\n").as_bytes())?,
        Some("view") => write_stdout(b"\"2.0.0\"\n")?,
        Some("install") => {}
        _ => leave(1),
    }
    leave(0)
}

/// Answer a `root` query from the prefix operand, or record the invocation.
fn prefixed(operands: &[OsString], suffix: &str, log: &str) -> io::Result<()> {
    let [option, prefix, rest @ ..] = operands else {
        return Err(io::Error::other("expected a prefix followed by a command"));
    };
    if option.as_os_str() != OsStr::new("--prefix") {
        return Err(io::Error::other("expected the `--prefix` option"));
    }
    if rest.first().is_some_and(|command| command == "root") {
        let mut root = prefix.as_encoded_bytes().to_vec();
        root.extend_from_slice(suffix.as_bytes());
        root.push(b'\n');
        write_stdout(&root)?;
        leave(0);
    }
    let mut line = Vec::new();
    for operand in operands {
        line.extend_from_slice(operand.as_encoded_bytes());
        line.push(b'\n');
    }
    append_bytes(log, &line)?;
    leave(0)
}

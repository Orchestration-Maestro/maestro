//! Programs a guest component runs through explicit execution, selected by a sibling JSON record.
//!
//! The record names the scenario; the component's literal operands carry the per-call values.

use std::ffi::OsString;
use std::io::{self, Read, Write};

use maestro_test_process::child::{connect, leave, receive, write_stderr, write_stdout};
use serde::Deserialize;

/// An output descriptor, never an arbitrary file descriptor.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Stream {
    /// Raw standard output.
    Stdout,
    /// Raw standard error.
    Stderr,
}

/// One write, which the test grants before the program makes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Chunk {
    /// The descriptor that receives these bytes.
    stream: Stream,
    /// Unmodified bytes, including a partial UTF-8 scalar.
    bytes: Vec<u8>,
}

/// Component execution scenarios, parsed completely before any file, stream or socket effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Write independent raw streams and end with a status.
    Emit {
        /// Standard output bytes.
        stdout: Vec<u8>,
        /// Standard error bytes.
        stderr: Vec<u8>,
        /// Exit status.
        code: u8,
    },
    /// Write the first two operands to the streams, announce `READY` on the third, a port,
    /// and wait for one release byte.
    Pending {},
    /// As `Pending`, but answer each termination request with `T` on the socket and stay alive.
    #[cfg(unix)]
    Stubborn {},
    /// Ask the test for a release on the port in the only operand before each write.
    Chunks {
        /// Ordered writes, all decoded before connecting.
        chunks: Vec<Chunk>,
    },
    /// Print the operands, the working directory and the length of standard input as JSON.
    Report {},
    /// Print the working directory without a newline.
    Cwd {},
    /// Create the file the only operand names.
    Touch {},
    /// Replace this process with a test binary rerunning one test.
    #[cfg(unix)]
    Rerun {
        /// The test executable to run.
        binary: String,
        /// The exact test name it runs.
        test: String,
    },
}

impl Record {
    /// Reject unknown scenarios and fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("component-exec\n")
            .ok_or_else(|| io::Error::other("missing component-exec family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// The two stream texts and the port a pending program takes as operands.
fn pending_operands(operands: &[OsString]) -> io::Result<(&OsString, &OsString, u16)> {
    let [stdout, stderr, port] = operands else {
        return Err(io::Error::other("expected two stream texts and a port"));
    };
    let port = port
        .to_str()
        .and_then(|port| port.parse().ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| io::Error::other("the port operand is not a nonzero port"))?;
    Ok((stdout, stderr, port))
}

/// Run the selected program with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Emit {
            stdout,
            stderr,
            code,
        } => {
            write_stdout(stdout)?;
            write_stderr(stderr)?;
            leave(i32::from(*code))
        }
        Record::Pending {} => {
            let (stdout, stderr, port) = pending_operands(operands)?;
            write_stdout(stdout.as_encoded_bytes())?;
            write_stderr(stderr.as_encoded_bytes())?;
            let mut socket = connect(port)?;
            socket.write_all(b"READY")?;
            receive(&mut socket)?;
            leave(0)
        }
        #[cfg(unix)]
        Record::Stubborn {} => stubborn(operands),
        Record::Chunks { chunks } => granted(chunks, operands),
        Record::Report {} => report(operands),
        Record::Cwd {} => write_stdout(std::env::current_dir()?.as_os_str().as_encoded_bytes()),
        Record::Touch {} => {
            let [path] = operands else {
                return Err(io::Error::other("expected one path"));
            };
            std::fs::File::create(path).map(drop)
        }
        #[cfg(unix)]
        Record::Rerun { binary, test } => {
            use std::os::unix::process::CommandExt;
            Err(std::process::Command::new(binary)
                .args([test, "--exact", "--nocapture"])
                .exec())
        }
    }
}

/// Answer each termination request with `T` while staying alive until released.
#[cfg(unix)]
fn stubborn(operands: &[OsString]) -> io::Result<()> {
    let (stdout, stderr, port) = pending_operands(operands)?;
    write_stdout(stdout.as_encoded_bytes())?;
    write_stderr(stderr.as_encoded_bytes())?;
    let mut socket = connect(port)?;
    let (mut notified, writer) = std::os::unix::net::UnixStream::pair()?;
    let _registration =
        signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer)?;
    let mut answers = socket.try_clone()?;
    std::thread::spawn(move || {
        while notified.read_exact(&mut [0]).is_ok() && answers.write_all(b"T").is_ok() {}
    });
    socket.write_all(b"READY")?;
    receive(&mut socket)?;
    leave(0)
}

/// Write each chunk after the test grants its release on the port operand.
fn granted(chunks: &[Chunk], operands: &[OsString]) -> io::Result<()> {
    let [port] = operands else {
        return Err(io::Error::other("expected one port"));
    };
    let port = port
        .to_str()
        .and_then(|port| port.parse().ok())
        .ok_or_else(|| io::Error::other("the port operand is not a port"))?;
    let mut socket = connect(port)?;
    for chunk in chunks {
        socket.write_all(b"R")?;
        if receive(&mut socket)? != Some(b'x') {
            return Err(io::Error::other("missing release"));
        }
        match chunk.stream {
            Stream::Stdout => write_stdout(&chunk.bytes)?,
            Stream::Stderr => write_stderr(&chunk.bytes)?,
        }
    }
    leave(0)
}

/// Print `[operands, cwd, stdin length]` as one JSON line.
fn report(operands: &[OsString]) -> io::Result<()> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    let operands: Vec<_> = operands
        .iter()
        .map(|operand| operand.to_string_lossy())
        .collect();
    let report = serde_json::json!([operands, std::env::current_dir()?, input.len()]);
    write_stdout(format!("{report}\n").as_bytes())
}

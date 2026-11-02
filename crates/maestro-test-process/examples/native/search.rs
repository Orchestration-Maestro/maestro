//! Finite native search and completion producers selected by a sibling JSON record.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::unix::{net::UnixStream, process::CommandExt};
use std::process::{Command, Stdio};

use maestro_test_process::child::{
    connect, leave, read_input, receive, write_stderr, write_stdout,
};
use rustix::process::{Signal, getpid, kill_process};
use serde::Deserialize;

/// The only signal a controlled search response sends itself.
#[derive(Deserialize)]
pub(super) enum ExitSignal {
    /// Terminate with the native signal rather than an exit code.
    #[serde(rename = "SIGTERM")]
    Terminate,
}

/// An output descriptor, never an arbitrary file descriptor.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Stream {
    /// Raw standard output.
    Stdout,
    /// Independent diagnostic bytes.
    Stderr,
}

/// One acknowledged write, including a possible partial UTF-8 scalar.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Chunk {
    /// The native stream that receives these bytes.
    stream: Stream,
    /// Unmodified bytes for this write.
    bytes: Vec<u8>,
}

/// Search scenarios parsed completely before any files, streams or sockets change.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Record literal operands and write independent raw streams.
    Response {
        /// Destination for the operand array.
        args: String,
        /// Standard output bytes, including incomplete records.
        stdout: Vec<u8>,
        /// Standard error bytes.
        stderr: Vec<u8>,
        /// Status when no signal is requested.
        code: u8,
        /// Native termination instead of a numeric exit.
        signal: Option<ExitSignal>,
    },
    /// Observe discovery's version probe independently of version text and status.
    Probe {
        /// Optional append-only probe log.
        log: Option<String>,
        /// Alias name included in the observation.
        name: String,
        /// Version bytes, which need not be UTF-8 or match the installed version.
        stdout: Vec<u8>,
        /// Native exit status after the probe.
        code: u8,
    },
    /// Demonstrate that an executable would create the injection/probe sentinel.
    Marker {
        /// Sentinel destination.
        path: String,
        /// Sentinel contents.
        text: String,
        /// Echo the first operand's file as a search preprocessor would.
        copy_input: bool,
    },
    /// Stay alive while discovery owns this process and its socket.
    ProbeWait {
        /// Parent-owned readiness channel.
        socket: String,
    },
    /// Acknowledge find's TERM while keeping output pipes open until release.
    FindTerm {
        /// Parent-owned readiness and release channel.
        socket: String,
    },
    /// Write one result and acknowledge limit/cancellation TERM before exiting.
    GrepTerm {
        /// Parent-owned readiness and release channel.
        socket: String,
        /// Complete search event bytes.
        event: Vec<u8>,
        /// Send a start acknowledgement after the event for the nonlimited case.
        started: bool,
    },
    /// Split output across independently acknowledged native stream writes.
    Chunks {
        /// Parent-owned readiness and release channel.
        socket: String,
        /// Ordered writes, all decoded before connecting.
        chunks: Vec<Chunk>,
        /// Exit status after the final write.
        code: u8,
    },
    /// Mark the configured shell choice, then execute the real shell with native operands.
    ChosenShell {},
    /// Write an incomplete record, announce readiness and wait for cancellation.
    CompletionWait {
        /// Standard output bytes, including a record that never ends.
        stdout: Vec<u8>,
        /// Parent-owned readiness port.
        port: u16,
    },
    /// Require a closed standard input, then fill the diagnostic pipe before standard output.
    CompletionStream {
        /// Count of repeating `x` bytes flushed to standard error first.
        stderr: u64,
        /// Standard output bytes written afterwards.
        stdout: Vec<u8>,
    },
    /// Report this process's id and exit while a linked descendant retains both pipes.
    CompletionDescendant {
        /// Parent-owned port for the process-id report.
        port: u16,
        /// Installed alias whose record holds the pipes after this process exits.
        holder: String,
    },
    /// Greet on the parent's port, then hold both inherited pipes until released.
    CompletionHolder {
        /// Parent-owned port for the greeting and release byte.
        port: u16,
    },
}

impl Record {
    /// Reject unknown scenarios, extra fields and malformed byte/status values before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("search\n")
            .ok_or_else(|| io::Error::other("missing search family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Keep native strings until the JSON observation boundary.
fn arguments(path: &str, operands: &[OsString]) -> io::Result<()> {
    let operands: Vec<_> = operands
        .iter()
        .map(|value| value.to_string_lossy())
        .collect();
    std::fs::write(path, serde_json::to_vec(&operands)?)
}

/// Write the selected response without harness bytes or a command parser.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Response {
            args,
            stdout,
            stderr,
            code,
            signal,
        } => {
            arguments(args, operands)?;
            write_stdout(stdout)?;
            write_stderr(stderr)?;
            if let Some(ExitSignal::Terminate) = signal {
                kill_process(getpid(), Signal::TERM)?;
            }
            leave(i32::from(*code))
        }
        Record::Probe {
            log,
            name,
            stdout,
            code,
        } => probe(log.as_deref(), name, stdout, *code, operands),
        Record::Marker {
            path,
            text,
            copy_input,
        } => marker(path, text, *copy_input, operands),
        Record::ProbeWait { socket } => {
            let mut socket = UnixStream::connect(socket)?;
            socket.write_all(b"ready")?;
            receive(&mut socket).map(|_| ())
        }
        Record::FindTerm { socket } => find_term(socket),
        Record::GrepTerm {
            socket,
            event,
            started,
        } => grep_term(socket, event, *started),
        Record::Chunks {
            socket,
            chunks,
            code,
        } => chunks_output(socket, chunks, *code),
        Record::ChosenShell {} => {
            write_stdout(b"chosen-shell\n")?;
            Err(std::process::Command::new("/bin/bash")
                .args(operands)
                .exec())
        }
        Record::CompletionWait { stdout, port } => completion_wait(stdout, *port),
        Record::CompletionStream { stderr, stdout } => completion_stream(*stderr, stdout),
        Record::CompletionDescendant { port, holder } => completion_descendant(*port, holder),
        Record::CompletionHolder { port } => completion_holder(*port),
    }
}

/// Write an incomplete record, announce readiness and wait for the consumer's cancellation.
fn completion_wait(stdout: &[u8], port: u16) -> io::Result<()> {
    write_stdout(stdout)?;
    let mut socket = connect(port)?;
    socket.write_all(b"READY")?;
    receive(&mut socket).map(|_| ())
}

/// Require the consumer's closed standard input, then fill the diagnostic pipe first.
fn completion_stream(stderr: u64, stdout: &[u8]) -> io::Result<()> {
    if !read_input()?.is_empty() {
        return Err(io::Error::other("expected a closed standard input"));
    }
    let count = usize::try_from(stderr).map_err(io::Error::other)?;
    let diagnostics = vec![b'x'; count];
    write_stderr(&diagnostics)?;
    write_stdout(stdout)
}

/// Report the native id, leave a descendant retaining both output pipes and exit.
fn completion_descendant(port: u16, holder: &str) -> io::Result<()> {
    let mut report = connect(port)?;
    report.write_all(std::process::id().to_string().as_bytes())?;
    drop(report);
    let descendant = Command::new(holder)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    // The descendant deliberately outlives this process and retains both output pipes.
    drop(descendant);
    leave(0)
}

/// Announce the descendant's hold, then release both pipes when the consumer is done.
fn completion_holder(port: u16) -> io::Result<()> {
    let mut socket = connect(port)?;
    socket.write_all(b"D")?;
    receive(&mut socket).map(|_| ())
}

/// Logs operands, requires `--version`, writes supplied bytes and exits with the supplied status.
fn probe(
    log: Option<&str>,
    name: &str,
    stdout: &[u8],
    code: u8,
    operands: &[OsString],
) -> io::Result<()> {
    if let Some(log) = log {
        let mut file = std::fs::File::options()
            .append(true)
            .create(true)
            .open(log)?;
        writeln!(
            file,
            "{name} {}",
            operands
                .iter()
                .map(|v| v.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        )?;
    }
    if operands != [OsString::from("--version")] {
        leave(99);
    }
    write_stdout(stdout)?;
    leave(i32::from(code))
}

/// Leave a visible sentinel if an operand is mistakenly executed instead of searched.
fn marker(path: &str, text: &str, copy_input: bool, operands: &[OsString]) -> io::Result<()> {
    std::fs::write(path, text)?;
    if copy_input {
        let path = operands
            .first()
            .ok_or_else(|| io::Error::other("missing preprocessor input"))?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut io::stdout())?;
    }
    Ok(())
}

/// A queued TERM notification and the registration that owns its write end.
struct Termination {
    /// Retained read end, receiving a byte even when TERM precedes the wait.
    reader: UnixStream,
    /// Handler removed before the read end closes on normal return.
    registration: signal_hook::SigId,
}

impl Termination {
    /// Consume the queued signal notification without polling or a deadline.
    fn wait(&mut self) -> io::Result<()> {
        self.reader.read_exact(&mut [0])
    }
}

impl Drop for Termination {
    /// Remove the handler before releasing the retained notification reader.
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}

/// Register TERM before readiness so an early signal is retained by the native pipe.
fn termination() -> io::Result<Termination> {
    let (reader, writer) = UnixStream::pair()?;
    let registration =
        signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer)?;
    Ok(Termination {
        reader,
        registration,
    })
}

/// Require the parent's release byte without timers or polling.
fn released(socket: &mut UnixStream) -> io::Result<()> {
    if receive(socket)? == Some(b'x') {
        Ok(())
    } else {
        Err(io::Error::other("missing parent release"))
    }
}

/// Wait for TERM, acknowledge it, and retain native pipe ownership until released.
fn find_term(path: &str) -> io::Result<()> {
    let mut signals = termination()?;
    let mut socket = UnixStream::connect(path)?;
    socket.write_all(b"ready")?;
    signals.wait()?;
    socket.write_all(b"term")?;
    receive(&mut socket).map(|_| ())
}

/// Release one event, then acknowledge termination while deliberately remaining alive.
fn grep_term(path: &str, event: &[u8], started: bool) -> io::Result<()> {
    let mut signals = termination()?;
    let mut socket = UnixStream::connect(path)?;
    socket.write_all(b"R")?;
    released(&mut socket)?;
    write_stdout(event)?;
    if started {
        socket.write_all(b"S")?;
    }
    signals.wait()?;
    socket.write_all(b"T")?;
    released(&mut socket)?;
    leave(9)
}

/// Acknowledge each byte chunk on a separate socket before writing to its chosen pipe.
fn chunks_output(path: &str, chunks: &[Chunk], code: u8) -> io::Result<()> {
    let mut socket = UnixStream::connect(path)?;
    for chunk in chunks {
        socket.write_all(b"R")?;
        released(&mut socket)?;
        match chunk.stream {
            Stream::Stdout => write_stdout(&chunk.bytes),
            Stream::Stderr => write_stderr(&chunk.bytes),
        }?;
    }
    leave(i32::from(code))
}

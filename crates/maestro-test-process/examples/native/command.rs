//! Finite controlled program stand-ins selected by a sibling JSON record.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;

use maestro_test_process::child::{leave, read_input, variable, write_stderr, write_stdout};
use rustix::process::{Signal, getpid, kill_process};
use serde::Deserialize;

/// Native termination instead of a numeric status.
#[derive(Deserialize)]
pub(super) enum ExitSignal {
    /// Terminate with the native SIGTERM.
    #[serde(rename = "SIGTERM")]
    Terminate,
    /// Terminate with the native SIGKILL, which leaves no exit code.
    #[serde(rename = "SIGKILL")]
    Kill,
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

/// Raw streams, a status and the effects one invocation performs first.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Streams {
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
    /// Write each standard output byte separately, so a reader must reassemble split UTF-8.
    #[serde(default)]
    split: bool,
}

impl Streams {
    /// Record the invocation, write both streams, then end with the status or signal.
    fn run(&self, operands: &[OsString]) -> io::Result<()> {
        if let Some(append) = &self.append {
            append_to(append)?;
        }
        if let Some(log) = &self.argv_log {
            record_argv(log, operands)?;
        }
        if self.split {
            for byte in &self.stdout {
                write_stdout(&[*byte])?;
            }
        } else {
            write_stdout(&self.stdout)?;
        }
        write_stderr(&self.stderr)?;
        match self.signal {
            Some(ExitSignal::Terminate) => kill_process(getpid(), Signal::TERM)?,
            Some(ExitSignal::Kill) => kill_process(getpid(), Signal::KILL)?,
            #[cfg(target_os = "linux")]
            Some(ExitSignal::Realtime) => {
                signal_hook::low_level::raise(REALTIME_SIGNAL).map_err(io::Error::other)?;
            }
            None => {}
        }
        leave(i32::from(self.code))
    }
}

/// One value a report prints, in the recorded order.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Field {
    /// The named variable's text, or nothing when it is unset.
    Variable(String),
    /// The absolute working directory.
    WorkingDirectory,
    /// One operand, by position.
    Operand(usize),
    /// The number of bytes standard input held.
    Input,
}

/// The manifest an install invocation writes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    /// Operand that must name the installed spec.
    argument: String,
    /// File written with the recorded contents.
    path: String,
    /// Complete manifest contents.
    contents: String,
}

/// A process that records readiness, blocks on a permit and then finishes when released.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hold {
    /// Marker written immediately before blocking, when a reader waits for it.
    ready: Option<String>,
    /// FIFO read after the marker, which returns when the test writes a byte to it.
    permit: String,
    /// FIFO read before the permit, whose writer may have opened it first.
    late: Option<String>,
    /// Bytes written to standard output after the gate.
    stdout: Vec<u8>,
    /// Bytes written to standard error after the gate.
    stderr: Vec<u8>,
    /// Append performed after the gate.
    append: Option<Append>,
    /// Append performed when this process receives SIGTERM; it then exits with status 0.
    terminate: Option<Append>,
}

/// Program scenarios, parsed completely before any file or stream effect.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Write independent raw streams and end with the selected status or signal.
    Output(Streams),
    /// Report the first operand on both streams, then echo one line of input.
    Labelled {},
    /// Print the recorded values in order, separated by `|`.
    Report {
        /// Values printed in order.
        fields: Vec<Field>,
    },
    /// Print the operands, the invoked name, one input line and the working directory.
    Stdio {},
    /// Answer package-manager queries from the recorded values.
    Npm {
        /// Text a `view` invocation prints.
        view: String,
        /// Root a `root` invocation prints.
        root: Option<String>,
        /// Manifest an `install` invocation writes.
        install: Option<Manifest>,
        /// Operand log each invocation appends to first.
        log: Option<ArgvLog>,
    },
    /// Record readiness, block on a permit, then finish or wait for termination.
    Hold(Hold),
    /// Start a descendant that holds the shared streams, then end at once.
    Holder {
        /// Bytes written to standard output before this process ends.
        stdout: Vec<u8>,
        /// Bytes written to standard error before this process ends.
        stderr: Vec<u8>,
        /// Installed alias whose own record assigns the descendant its work.
        descendant: String,
    },
    /// Record the invocation, then replace this process with the recorded program.
    Forward {
        /// Append performed before the program runs.
        marker: Append,
        /// Program that receives the untouched operands.
        program: String,
    },
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
}

impl Record {
    /// Reject unknown scenarios and extra fields before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("command\n")
            .ok_or_else(|| io::Error::other("missing command family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected stand-in with its untouched native operands.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Output(streams) => streams.run(operands),
        Record::Labelled {} => labelled(operands),
        Record::Report { fields } => report(fields, operands),
        Record::Stdio {} => stdio(operands),
        Record::Npm {
            view,
            root,
            install,
            log,
        } => npm(
            view,
            root.as_deref(),
            install.as_ref(),
            log.as_ref(),
            operands,
        ),
        Record::Hold(hold) => Hold::run(hold),
        Record::Holder {
            stdout,
            stderr,
            descendant,
        } => holder(stdout, stderr, descendant),
        Record::Forward { marker, program } => forward(marker, program, operands),
        Record::Query {
            prefix,
            denied,
            error,
            code,
        } => query(operands, prefix, denied, error, *code),
        Record::Package {} => package(operands),
    }
}

/// Print each recorded value, joined by `|`.
fn report(fields: &[Field], operands: &[OsString]) -> io::Result<()> {
    let mut input = None;
    let mut values = Vec::new();
    for field in fields {
        values.push(match field {
            Field::Variable(name) => std::env::var(name).unwrap_or_default(),
            Field::WorkingDirectory => std::env::current_dir()?.to_string_lossy().into_owned(),
            Field::Operand(index) => operands
                .get(*index)
                .map(|operand| operand.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Field::Input => {
                let bytes = match &input {
                    Some(bytes) => bytes,
                    None => input.insert(read_input()?),
                };
                bytes.len().to_string()
            }
        });
    }
    write_stdout(values.join("|").as_bytes())
}

/// Print the operands, the invoked name, one input line and the working directory.
fn stdio(operands: &[OsString]) -> io::Result<()> {
    let mut operands_text = String::new();
    for operand in operands {
        operands_text.push('<');
        operands_text.push_str(&operand.to_string_lossy());
        operands_text.push('>');
    }
    let invoked = std::env::args_os().next().unwrap_or_default();
    let name = std::path::Path::new(&invoked)
        .file_name()
        .unwrap_or(invoked.as_os_str());
    let name = name.to_string_lossy();
    write_stdout(format!("{operands_text}out-{name}\n").as_bytes())?;
    write_stderr(format!("err-{name}\n").as_bytes())?;
    let line = read_line()?;
    write_stdout(format!("stdin:{}\n", String::from_utf8_lossy(&line)).as_bytes())?;
    write_stdout(format!("{}\n", std::env::current_dir()?.to_string_lossy()).as_bytes())
}

/// Answer a package-manager query, or fail for an invocation the record does not admit.
fn npm(
    view: &str,
    root: Option<&str>,
    install: Option<&Manifest>,
    log: Option<&ArgvLog>,
    operands: &[OsString],
) -> io::Result<()> {
    if let Some(log) = log {
        record_argv(log, operands)?;
    }
    match operands.first().and_then(|command| command.to_str()) {
        Some("view") => {
            write_stdout(view.as_bytes())?;
            leave(0)
        }
        Some("root") => {
            write_stdout(root.unwrap_or_default().as_bytes())?;
            leave(0)
        }
        Some("install") => write_manifest(install, operands),
        _ => leave(1),
    }
}

/// Write the recorded manifest once the second operand names the installed spec.
fn write_manifest(install: Option<&Manifest>, operands: &[OsString]) -> io::Result<()> {
    let install = install.ok_or_else(|| io::Error::other("no recorded install"))?;
    if operands.get(1).map(OsString::as_os_str) != Some(OsStr::new(&install.argument)) {
        leave(1);
    }
    std::fs::write(&install.path, &install.contents)?;
    leave(0)
}

/// Start the descendant on a shared input pipe, print, and end before it does.
fn holder(stdout: &[u8], stderr: &[u8], descendant: &str) -> io::Result<()> {
    let mut child = std::process::Command::new(descendant)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;
    // The descendant starts once this process ends, which closes its inherited input.
    let _held = child.stdin.take();
    write_stdout(stdout)?;
    write_stderr(stderr)?;
    leave(0)
}

/// Append the marker, then replace this process with the recorded program.
fn forward(marker: &Append, program: &str, operands: &[OsString]) -> io::Result<()> {
    append_to(marker)?;
    Err(std::process::Command::new(program).args(operands).exec())
}

impl Hold {
    /// Drain the shared input, announce readiness, wait for the release and report.
    fn run(&self) -> io::Result<()> {
        let termination = self.terminate.as_ref().map(|_| termination()).transpose()?;
        drop(read_input()?);
        if let Some(ready) = &self.ready {
            std::fs::write(ready, b"ready")?;
        }
        if let Some(late) = &self.late {
            wait_for_release(late)?;
        }
        wait_for_release(&self.permit)?;
        write_stdout(&self.stdout)?;
        write_stderr(&self.stderr)?;
        if let Some(append) = &self.append {
            append_to(append)?;
        }
        if let (Some(mut signal), Some(terminate)) = (termination, &self.terminate) {
            // The handler writes into the pipe, so this wait blocks until SIGTERM arrives.
            signal.read_exact(&mut [0; 1])?;
            append_to(terminate)?;
        }
        leave(0)
    }
}

/// A read end that becomes readable when this process's SIGTERM handler writes one byte.
fn termination() -> io::Result<UnixStream> {
    let (reader, writer) = UnixStream::pair()?;
    signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer)
        .map_err(io::Error::other)?;
    Ok(reader)
}

/// Blocks until the test writes one byte, which is the only release a gate waits for.
fn wait_for_release(path: &str) -> io::Result<()> {
    let mut release = std::fs::File::open(path)?;
    release.read_exact(&mut [0; 1])
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
